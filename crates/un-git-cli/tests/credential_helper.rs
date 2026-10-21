//! Actual checked Git -> shell -> CLI helper -> private socket roundtrip.
//! Uses only a generated token-shaped fixture; no Keychain or network access.
use std::{
    io::{BufRead, BufReader, Write},
    os::{
        fd::AsRawFd,
        unix::{net::UnixStream, process::CommandExt},
    },
    path::Path,
    process::{Command, Stdio},
    time::Duration,
};

const TOKEN: &str = "ghu_generated_integration_fixture";

#[test]
fn actual_cli_helper_uses_only_inherited_parent_channel() {
    let root = tempfile::tempdir().unwrap();
    let executable = root.path().join("helper with spaces and 'quote");
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_un-git"), &executable).unwrap();
    let (mut server, inherited) = UnixStream::pair().unwrap();
    server
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let worker = std::thread::spawn(move || {
        let mut request = String::new();
        BufReader::new(server.try_clone().unwrap())
            .read_line(&mut request)
            .unwrap();
        let request: serde_json::Value = serde_json::from_str(&request).unwrap();
        assert_eq!(
            request,
            serde_json::json!({"version":1,"protocol":"https","host":"github.com","path":"fixture/repository.git"})
        );
        writeln!(server, "{}", serde_json::json!({"password":TOKEN})).unwrap();
    });
    let git = [
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/usr/bin/git",
    ]
    .into_iter()
    .find(|p| Path::new(p).is_file())
    .unwrap();
    let fd = inherited.as_raw_fd();
    let helper = format!(
        "!'{}' --un-git-internal-credential-helper",
        executable.to_str().unwrap().replace('\'', "'\\''")
    );
    let mut command = Command::new(git);
    command
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("UN_GIT_CREDENTIAL_SOCKET_FD", fd.to_string())
        .args([
            "-c",
            "credential.helper=",
            "-c",
            "credential.useHttpPath=true",
            "-c",
        ])
        .arg(format!("credential.helper={helper}"))
        .args(["credential", "fill"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    unsafe {
        command.pre_exec(move || {
            if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"protocol=https\nhost=github.com\npath=fixture/repository.git\n\n")
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Git could not use the CLI helper: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    worker.join().unwrap();
    assert!(output.stderr.is_empty());
    assert!(output
        .stdout
        .windows(TOKEN.len())
        .any(|bytes| bytes == TOKEN.as_bytes()));
    // Nothing except the symlink was persisted, and no application was opened.
    assert_eq!(std::fs::read_dir(root.path()).unwrap().count(), 1);
}

#[test]
fn standalone_helper_cannot_read_keychain_or_emit_credentials() {
    let output = Command::new(env!("CARGO_BIN_EXE_un-git"))
        .env_clear()
        .args(["--un-git-internal-credential-helper", "get"])
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
