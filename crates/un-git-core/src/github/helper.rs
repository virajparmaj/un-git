//! A one-process credential channel. No Keychain access exists in helper mode.
use crate::{CoreError, Result};
use serde::{Deserialize, Serialize};
use std::{
    io::{self, Read, Write},
    net::Shutdown,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::Command,
    thread::JoinHandle,
    time::Duration,
};

const MODE: &str = "--un-git-internal-credential-helper";
const FD_VARIABLE: &str = "UN_GIT_CREDENTIAL_SOCKET_FD";
const MESSAGE_LIMIT: usize = 8192;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    version: u32,
    protocol: String,
    host: String,
    path: String,
}

// Deliberately no Debug: this response contains an in-memory credential.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Response {
    password: String,
}

fn helper_error() -> CoreError {
    CoreError::new(
        "GIT_CREDENTIAL_CHANNEL",
        "The private Git credential channel is unavailable.",
    )
}

fn read_line(reader: &mut impl Read, maximum: usize) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        if reader.read(&mut byte)? == 0 {
            // Git closes helper stdin after the final field rather than always
            // writing a blank line. Required fields and JSON framing are still
            // validated by the caller; empty socket EOF cannot yield a token.
            return Ok(line);
        }
        if byte[0] == b'\n' {
            return Ok(line);
        }
        if line.len() >= maximum {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Credential request is too large",
            ));
        }
        line.push(byte[0]);
    }
}

fn read_request(input: &mut impl Read) -> Result<Request> {
    let mut protocol = None;
    let mut host = None;
    let mut path = None;
    let mut consumed = 0usize;
    loop {
        let line =
            read_line(input, MESSAGE_LIMIT.saturating_sub(consumed)).map_err(|_| helper_error())?;
        consumed += line.len() + 1;
        if consumed > MESSAGE_LIMIT {
            return Err(helper_error());
        }
        if line.is_empty() {
            break;
        }
        let line = std::str::from_utf8(&line).map_err(|_| helper_error())?;
        let (key, value) = line.split_once('=').ok_or_else(helper_error)?;
        if value.contains(['\r', '\0']) {
            return Err(helper_error());
        }
        let slot = match key {
            "protocol" => &mut protocol,
            "host" => &mut host,
            "path" => &mut path,
            "username" if value == "x-access-token" || value.is_empty() => continue,
            "wwwauth[]" => continue,
            // Git versions may announce credential capabilities. We emit only
            // ordinary username/password fields and never accept alternate URLs.
            "capability[]" => continue,
            _ => return Err(helper_error()),
        };
        if slot.replace(value.to_owned()).is_some() {
            return Err(helper_error());
        }
    }
    Ok(Request {
        version: 1,
        protocol: protocol.ok_or_else(helper_error)?,
        host: host.ok_or_else(helper_error)?,
        path: path.ok_or_else(helper_error)?,
    })
}

fn handle_get(
    input: &mut impl Read,
    output: &mut impl Write,
    socket: &mut UnixStream,
) -> Result<()> {
    let request = read_request(input)?;
    let mut encoded = serde_json::to_vec(&request).map_err(|_| helper_error())?;
    encoded.push(b'\n');
    socket.write_all(&encoded).map_err(|_| helper_error())?;
    let bytes = read_line(socket, MESSAGE_LIMIT).map_err(|_| helper_error())?;
    let response: Response = serde_json::from_slice(&bytes).map_err(|_| helper_error())?;
    if !valid_token(&response.password) {
        return Err(helper_error());
    }
    write!(
        output,
        "username=x-access-token\npassword={}\n\n",
        response.password
    )
    .map_err(|_| helper_error())?;
    output.flush().map_err(|_| helper_error())
}

fn valid_token(token: &str) -> bool {
    token.starts_with("ghu_")
        && token.len() <= 1024
        && token
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

/// Call before normal CLI parsing or desktop initialization. The mode cannot
/// read Keychain: it works only with an inherited parent-created socket endpoint.
pub fn run_credential_helper_if_requested() -> Option<i32> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.get(1).is_none_or(|argument| argument != MODE) {
        return None;
    }
    if args.len() != 3 {
        return Some(1);
    }
    let operation = args[2].to_str().unwrap_or("");
    if matches!(operation, "store" | "erase") {
        // Git can pass the token back when reporting success/failure. Never log
        // or persist it, and never let a transport failure revoke account auth.
        let result = io::copy(
            &mut io::stdin().take(MESSAGE_LIMIT as u64 + 1),
            &mut io::sink(),
        );
        return Some(if result.is_ok_and(|n| n <= MESSAGE_LIMIT as u64) {
            0
        } else {
            1
        });
    }
    if operation != "get" {
        return Some(1);
    }
    let result = (|| -> Result<()> {
        let fd: i32 = std::env::var(FD_VARIABLE)
            .ok()
            .and_then(|s| s.parse().ok())
            .filter(|fd| *fd >= 3)
            .ok_or_else(helper_error)?;
        // Duplicate ownership rather than close a descriptor owned by another
        // subsystem. A regular file/pipe cannot masquerade as this socket.
        let duplicated = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
        if duplicated < 0 {
            return Err(helper_error());
        }
        let mut socket = unsafe { UnixStream::from_raw_fd(duplicated) };
        if !socket.peer_addr().map_err(|_| helper_error())?.is_unnamed() {
            return Err(helper_error());
        }
        socket.set_read_timeout(Some(Duration::from_secs(10)))?;
        socket.set_write_timeout(Some(Duration::from_secs(10)))?;
        handle_get(&mut io::stdin(), &mut io::stdout(), &mut socket)
    })();
    // No diagnostics in helper mode: neither Git input nor credentials may leak.
    Some(if result.is_ok() { 0 } else { 1 })
}

pub(super) struct CredentialBroker {
    socket: UnixStream,
    inherited: UnixStream,
    worker: Option<JoinHandle<()>>,
}

impl CredentialBroker {
    pub(super) fn new(expected_path: String, token: String) -> Result<Self> {
        if !valid_token(&token) {
            return Err(helper_error());
        }
        let (socket, inherited) = UnixStream::pair()?;
        let mut server = socket.try_clone()?;
        server.set_read_timeout(Some(Duration::from_secs(180)))?;
        server.set_write_timeout(Some(Duration::from_secs(10)))?;
        let worker = std::thread::spawn(move || {
            // Git may retry a credential challenge, but cannot turn this into
            // an unbounded credential server or request a different repository.
            for _ in 0..4 {
                let Ok(line) = read_line(&mut server, MESSAGE_LIMIT) else {
                    break;
                };
                let Ok(request) = serde_json::from_slice::<Request>(&line) else {
                    break;
                };
                if request.version != 1
                    || request.protocol != "https"
                    || request.host != "github.com"
                    || request.path != expected_path
                {
                    break;
                }
                let Ok(mut response) = serde_json::to_vec(&Response {
                    password: token.clone(),
                }) else {
                    break;
                };
                response.push(b'\n');
                if server.write_all(&response).is_err() {
                    break;
                }
            }
            let _ = server.shutdown(Shutdown::Both);
        });
        Ok(Self {
            socket,
            inherited,
            worker: Some(worker),
        })
    }

    pub(super) fn configure(&self, command: &mut Command, executable: &Path) -> Result<()> {
        let helper = helper_command(executable)?;
        self.inherit(command);
        command.args(["-c", "credential.helper="]);
        command.arg("-c").arg(format!("credential.helper={helper}"));
        Ok(())
    }

    fn inherit(&self, command: &mut Command) {
        let fd = self.inherited.as_raw_fd();
        command.env(FD_VARIABLE, fd.to_string());
        // Clear CLOEXEC only in the child. Never leak the channel into unrelated
        // commands started by another Rust thread.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

impl Drop for CredentialBroker {
    fn drop(&mut self) {
        let _ = self.socket.shutdown(Shutdown::Both);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn helper_command(executable: &Path) -> Result<String> {
    let executable = executable
        .to_str()
        .filter(|s| !s.contains(['\n', '\r', '\0']))
        .ok_or_else(helper_error)?;
    if !Path::new(executable).is_absolute() {
        return Err(helper_error());
    }
    // Git runs even absolute credential helpers through its shell. A leading
    // shell snippet allows correct single-quote escaping for spaces/apostrophes.
    Ok(format!("!'{}' {MODE}", executable.replace('\'', "'\\''")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::runner::{git_executable, isolated};
    use std::process::Stdio;

    const TOKEN: &str = "ghu_generated_fixture_only";

    #[test]
    fn private_channel_supplies_only_exact_repository_credentials() {
        let broker = CredentialBroker::new("fixture/repo.git".into(), TOKEN.into()).unwrap();
        let mut socket = broker.inherited.try_clone().unwrap();
        let mut output = Vec::new();
        handle_get(
            &mut &b"protocol=https\nhost=github.com\npath=fixture/repo.git\n\n"[..],
            &mut output,
            &mut socket,
        )
        .unwrap();
        assert_eq!(
            output,
            format!("username=x-access-token\npassword={TOKEN}\n\n").as_bytes()
        );
        assert!(handle_get(
            &mut &b"protocol=https\nhost=github.com\npath=other/repo.git\n\n"[..],
            &mut Vec::new(),
            &mut socket
        )
        .is_err());
    }

    #[test]
    fn helper_rejects_ambiguous_oversized_or_injected_inputs() {
        assert!(read_request(&mut &b"protocol=https\nhost=github.com\npath=x/y.git\n"[..]).is_ok());
        assert!(read_request(&mut &b"protocol=https\nhost=github.com\npath=x/y.git"[..]).is_ok());
        assert!(read_request(&mut &b"protocol=https\nhost=github.com\npath"[..]).is_err());
        for input in [
            "protocol=https\nhost=github.com\nhost=elsewhere\npath=x/y.git\n\n",
            "protocol=https\nhost=github.com\npath=x/y.git\r\n\n",
            "protocol=https\nhost=github.com\nurl=https://elsewhere\npath=x/y.git\n\n",
            "protocol=https\nhost=github.com\npath=x/y.git\npassword=unexpected\n\n",
        ] {
            assert!(read_request(&mut input.as_bytes()).is_err());
        }
        assert!(read_request(&mut vec![b'x'; MESSAGE_LIMIT + 1].as_slice()).is_err());
        assert_eq!(
            helper_command(Path::new("/tmp/space and 'quote/app")).unwrap(),
            "!'/tmp/space and '\\''quote/app' --un-git-internal-credential-helper"
        );
    }

    #[test]
    fn checked_git_preserves_anonymous_descriptor_through_credential_helper() {
        // Exercise the actual checked Git->shell descriptor inheritance. The
        // Rust parser/broker protocol is independently tested above. No network.
        let (mut server, inherited) = UnixStream::pair().unwrap();
        let worker = std::thread::spawn(move || {
            server
                .write_all(format!("username=x-access-token\npassword={TOKEN}\n\n").as_bytes())
                .unwrap();
        });
        let broker = CredentialBroker {
            socket: inherited.try_clone().unwrap(),
            inherited,
            worker: Some(worker),
        };
        let mut command = Command::new(git_executable().unwrap());
        isolated(&mut command);
        broker.inherit(&mut command);
        command.args([
            "-c",
            "credential.helper=",
            "-c",
            "credential.useHttpPath=true",
            "-c",
            "credential.helper=!f() { cat <&$UN_GIT_CREDENTIAL_SOCKET_FD; }; f",
            "credential",
            "fill",
        ]);
        command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = command.spawn().unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(b"protocol=https\nhost=github.com\npath=fixture/repo.git\n\n")
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "Checked Git did not preserve the private descriptor"
        );
        assert!(output
            .stdout
            .windows(TOKEN.len())
            .any(|w| w == TOKEN.as_bytes()));
        assert!(output.stderr.is_empty());
    }
}
