use crate::{CoreError, Result};
use sha2::{Digest, Sha256};
use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const OUTPUT_LIMIT: u64 = 128 * 1024 * 1024;

pub fn git_executable() -> Result<PathBuf> {
    // No PATH lookup: repository-controlled executables must never be selected.
    for path in [
        "/opt/homebrew/bin/git",
        "/usr/local/bin/git",
        "/usr/bin/git",
    ] {
        if Path::new(path).is_file() {
            return Ok(PathBuf::from(path));
        }
    }
    Err(CoreError::blocked("A checked Git runtime is required"))
}

pub fn python_executable() -> Result<PathBuf> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.venv/bin/python3");
    if project.is_file() {
        return Ok(project);
    }
    Err(CoreError::blocked(
        "The project Python environment is missing. Run npm run setup:worker before local preparation.",
    ))
}

pub fn worker_dir() -> PathBuf {
    if let Ok(executable) = std::env::current_exe() {
        if let Some(directory) = executable.parent() {
            let bundled = directory.join("../Resources/rewrite");
            if bundled.join("worker.py").is_file() {
                return bundled;
            }
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../workers/rewrite")
}

pub fn python_version() -> Result<String> {
    let mut command = Command::new(python_executable()?);
    isolated(&mut command);
    command.args(["-I", "--version"]);
    let version = String::from_utf8_lossy(&execute(command, None)?)
        .trim()
        .to_string();
    let numbers = version
        .strip_prefix("Python ")
        .unwrap_or("")
        .split('.')
        .take(2)
        .map(str::parse::<u32>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|_| CoreError::blocked("Cannot determine the project Python version"))?;
    if numbers.len() != 2 || numbers[0] != 3 || numbers[1] < 11 {
        return Err(CoreError::blocked(
            "The project worker requires Python 3.11 or later in the Python 3 series",
        ));
    }
    Ok(version)
}

pub fn isolated(command: &mut Command) {
    command
        .env_clear()
        .env("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin")
        .env("LC_ALL", "C")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_NO_REPLACE_OBJECTS", "1")
        .env("GIT_ATTR_NOSYSTEM", "1")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_CONFIG_COUNT", "7")
        .env("GIT_CONFIG_KEY_0", "core.hooksPath")
        .env("GIT_CONFIG_VALUE_0", "/dev/null")
        .env("GIT_CONFIG_KEY_1", "credential.helper")
        .env("GIT_CONFIG_VALUE_1", "")
        .env("GIT_CONFIG_KEY_2", "core.fsmonitor")
        .env("GIT_CONFIG_VALUE_2", "false")
        .env("GIT_CONFIG_KEY_3", "core.attributesFile")
        .env("GIT_CONFIG_VALUE_3", "/dev/null")
        .env("GIT_CONFIG_KEY_4", "commit.gpgSign")
        .env("GIT_CONFIG_VALUE_4", "false")
        .env("GIT_CONFIG_KEY_5", "tag.gpgSign")
        .env("GIT_CONFIG_VALUE_5", "false")
        .env("GIT_CONFIG_KEY_6", "protocol.ext.allow")
        .env("GIT_CONFIG_VALUE_6", "never");
}

pub fn execute(command: Command, input: Option<Vec<u8>>) -> Result<Vec<u8>> {
    execute_with_limits(command, input, Duration::from_secs(180), OUTPUT_LIMIT)
}

pub(crate) struct NetworkLimits {
    pub directory: PathBuf,
    pub maximum_bytes: u64,
    pub minimum_free_bytes: u64,
}

impl NetworkLimits {
    pub(crate) fn check(&self) -> Result<()> {
        let mut pending = vec![self.directory.clone()];
        let mut bytes = 0u64;
        let mut entries = 0usize;
        while let Some(path) = pending.pop() {
            let metadata = match std::fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                // Git may atomically rename pack/index temporary files while checked.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                Err(_) => return Err(CoreError::blocked("Cannot inspect import storage.")),
            };
            entries += 1;
            if entries > 50_000 || metadata.file_type().is_symlink() {
                return Err(CoreError::blocked(
                    "Import storage exceeds supported file limits.",
                ));
            }
            if metadata.is_dir() {
                match std::fs::read_dir(&path) {
                    Ok(children) => {
                        for child in children {
                            pending.push(child?.path());
                        }
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
                    Err(_) => return Err(CoreError::blocked("Cannot inspect import storage.")),
                }
            } else if metadata.is_file() {
                bytes = bytes
                    .checked_add(metadata.len())
                    .ok_or_else(|| CoreError::blocked("Import storage size overflow."))?;
                if bytes > self.maximum_bytes {
                    return Err(CoreError::blocked(
                        "Repository download exceeds the 1 GiB preview limit.",
                    ));
                }
            } else {
                return Err(CoreError::blocked(
                    "Import storage contains an unsupported file.",
                ));
            }
        }
        if fs2::available_space(&self.directory)? < self.minimum_free_bytes {
            return Err(CoreError::blocked(
                "Repository download stopped to preserve free disk space.",
            ));
        }
        Ok(())
    }
}

/// Separate entry point for fixed read-only network commands. Never return remote stderr.
pub(crate) fn execute_network(command: Command, limits: &NetworkLimits) -> Result<Vec<u8>> {
    execute_monitored(
        command,
        None,
        Duration::from_secs(180),
        8 * 1024 * 1024,
        Some(limits),
    )
}

fn execute_with_limits(
    command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
    output_limit: u64,
) -> Result<Vec<u8>> {
    execute_monitored(command, input, timeout, output_limit, None)
}

fn execute_monitored(
    mut command: Command,
    input: Option<Vec<u8>>,
    timeout: Duration,
    output_limit: u64,
    network_limits: Option<&NetworkLimits>,
) -> Result<Vec<u8>> {
    // A worker can exit while its Git descendants still own the pipes. The
    // deadline covers process exit AND pipe completion, not just the parent.
    command.process_group(0);
    if let Some(limits) = network_limits {
        limits.check()?;
        let maximum_bytes = limits.maximum_bytes;
        // Bound each file as well as monitoring aggregate usage. The aggregate
        // watchdog can overshoot between checks; it is not a filesystem quota.
        unsafe {
            command.pre_exec(move || {
                let limit = libc::rlimit {
                    rlim_cur: maximum_bytes as libc::rlim_t,
                    rlim_max: maximum_bytes as libc::rlim_t,
                };
                if libc::setrlimit(libc::RLIMIT_FSIZE, &limit) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| CoreError::local(format!("Cannot start checked process: {e}")))?;
    enum Event {
        Input(std::io::Result<()>),
        Output(std::io::Result<Vec<u8>>),
        Error(std::io::Result<Vec<u8>>),
    }
    let (sender, receiver) = std::sync::mpsc::channel();
    let mut stdin = child.stdin.take().expect("piped stdin");
    let input_sender = sender.clone();
    let writer = std::thread::spawn(move || {
        let result = input.map_or(Ok(()), |bytes| stdin.write_all(&bytes));
        drop(stdin);
        let _ = input_sender.send(Event::Input(result));
    });
    let stdout = child.stdout.take().expect("piped stdout");
    let output_sender = sender.clone();
    let out_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stdout
            .take(output_limit + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = output_sender.send(Event::Output(result));
    });
    let stderr = child.stderr.take().expect("piped stderr");
    let err_reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr
            .take(output_limit + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes);
        let _ = sender.send(Event::Error(result));
    });
    let deadline = Instant::now() + timeout;
    let mut status = None;
    let mut input_result = None;
    let mut out = None;
    let mut err = None;
    let mut next_storage_check = Instant::now();
    let outcome = loop {
        if Instant::now() >= next_storage_check {
            if let Some(limits) = network_limits {
                if let Err(error) = limits.check() {
                    break Err(error);
                }
            }
            next_storage_check = Instant::now() + Duration::from_millis(100);
        }
        if status.is_none() {
            match child.try_wait() {
                Ok(value) => status = value,
                Err(error) => break Err(CoreError::from(error)),
            }
        }
        if status.is_some() && input_result.is_some() && out.is_some() && err.is_some() {
            break Ok(());
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            break Err(CoreError::local(
                "Checked process exceeded its execution deadline",
            ));
        };
        match receiver.recv_timeout(remaining.min(Duration::from_millis(10))) {
            Ok(Event::Input(value)) => input_result = Some(value),
            Ok(Event::Output(value)) => {
                if value
                    .as_ref()
                    .is_ok_and(|bytes| bytes.len() as u64 > output_limit)
                {
                    break Err(CoreError::blocked(
                        "Process output exceeds the prototype safety limit",
                    ));
                }
                out = Some(value);
            }
            Ok(Event::Error(value)) => {
                if value
                    .as_ref()
                    .is_ok_and(|bytes| bytes.len() as u64 > output_limit)
                {
                    break Err(CoreError::blocked(
                        "Process output exceeds the prototype safety limit",
                    ));
                }
                err = Some(value);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                if input_result.is_none() || out.is_none() || err.is_none() {
                    break Err(CoreError::local("Process I/O worker failed"));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
        }
    };
    if outcome.is_err() {
        // Every child starts a private process group. Kill descendants even if
        // the direct child has already exited and was reaped by try_wait().
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = writer.join();
    let _ = out_reader.join();
    let _ = err_reader.join();
    outcome?;
    let out = out.expect("completed stdout")?;
    let err = err.expect("completed stderr")?;
    if !status.expect("completed child").success() {
        if network_limits.is_some() {
            return Err(CoreError::new("GIT_READ_FAILED", "GitHub repository download failed. Check repository access and network connectivity, then retry. No remote changes were requested."));
        }
        let detail = String::from_utf8_lossy(&err);
        return Err(CoreError::local(format!(
            "Checked process failed: {}",
            detail.chars().take(1500).collect::<String>()
        )));
    }
    input_result.expect("completed stdin")?;
    Ok(out)
}

pub fn git<I, S>(repository: Option<&Path>, args: I, input: Option<Vec<u8>>) -> Result<Vec<u8>>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = Command::new(git_executable()?);
    isolated(&mut command);
    if let Some(path) = repository {
        command.arg("-C").arg(path);
    }
    command.args(args);
    execute(command, input)
}

pub fn git_text(repository: Option<&Path>, args: &[&str]) -> Result<String> {
    let bytes = git(repository, args, None)?;
    String::from_utf8(bytes)
        .map(|s| s.trim_end().to_owned())
        .map_err(|_| CoreError::blocked("Git returned non-UTF-8 structural output"))
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
pub fn file_digest(path: &Path) -> Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

pub fn checked_oid(oid: &str) -> bool {
    (oid.len() == 40 || oid.len() == 64)
        && oid
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

pub fn batch(repository: &Path, oids: &[String]) -> Result<Vec<Vec<u8>>> {
    if oids.iter().any(|oid| !checked_oid(oid)) {
        return Err(CoreError::invalid("Invalid object identifier"));
    }
    let input = format!("{}\n", oids.join("\n")).into_bytes();
    if oids.is_empty() {
        return Ok(Vec::new());
    }
    // Preflight sizes before cat-file can allocate object payloads. One batch
    // metadata request avoids a subprocess per object on large histories.
    let metadata = git(
        Some(repository),
        ["cat-file", "--batch-check=%(objectname) %(objectsize)"],
        Some(input.clone()),
    )?;
    let metadata = std::str::from_utf8(&metadata)
        .map_err(|_| CoreError::blocked("Invalid object metadata"))?;
    let rows: Vec<_> = metadata.lines().collect();
    if rows.len() != oids.len() {
        return Err(CoreError::blocked("Incomplete object size inventory"));
    }
    let mut total = 0u64;
    for (oid, row) in oids.iter().zip(rows) {
        let (actual, size) = row
            .split_once(' ')
            .ok_or_else(|| CoreError::blocked("Invalid object size inventory"))?;
        let size: u64 = size
            .parse()
            .map_err(|_| CoreError::blocked("Missing or unsupported object in batch"))?;
        if actual != oid || size > 16 * 1024 * 1024 {
            return Err(CoreError::blocked(
                "Object exceeds the 16 MiB metadata payload limit",
            ));
        }
        total = total
            .checked_add(size)
            .ok_or_else(|| CoreError::blocked("Object byte count overflow"))?;
        if total > 64 * 1024 * 1024 {
            return Err(CoreError::blocked(
                "Object batch exceeds the 64 MiB metadata payload limit",
            ));
        }
    }
    let bytes = git(Some(repository), ["cat-file", "--batch"], Some(input))?;
    let mut cursor = 0;
    let mut objects = Vec::with_capacity(oids.len());
    for oid in oids {
        let newline = bytes[cursor..]
            .iter()
            .position(|b| *b == b'\n')
            .ok_or_else(|| CoreError::local("Truncated batch header"))?
            + cursor;
        let header = std::str::from_utf8(&bytes[cursor..newline])
            .map_err(|_| CoreError::local("Invalid batch header"))?;
        let fields: Vec<&str> = header.split(' ').collect();
        if fields.len() != 3 || fields[0] != oid {
            return Err(CoreError::local("Unexpected object batch response"));
        }
        let size = fields[2]
            .parse::<usize>()
            .map_err(|_| CoreError::local("Invalid object size"))?;
        let start = newline + 1;
        let end = start
            .checked_add(size)
            .ok_or_else(|| CoreError::local("Object size overflow"))?;
        if bytes.get(end) != Some(&b'\n') {
            return Err(CoreError::local("Truncated object batch"));
        }
        objects.push(bytes[start..end].to_vec());
        cursor = end + 1;
    }
    if cursor != bytes.len() {
        return Err(CoreError::local("Unexpected trailing object data"));
    }
    Ok(objects)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadline_covers_descendants_that_outlive_their_parent() {
        let mut command = Command::new("/bin/sh");
        isolated(&mut command);
        command.args(["-c", "sleep 10 & exit 0"]);
        let start = Instant::now();
        let error =
            execute_with_limits(command, None, Duration::from_millis(100), 1024).unwrap_err();
        assert!(error.message.contains("deadline"));
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn output_limit_stops_process_group_without_waiting_for_timeout() {
        let mut command = Command::new("/bin/sh");
        isolated(&mut command);
        command.args(["-c", "while :; do printf '0123456789'; done"]);
        let start = Instant::now();
        let error = execute_with_limits(command, None, Duration::from_secs(5), 1024).unwrap_err();
        assert_eq!(error.code, "BLOCKED");
        assert!(start.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn batch_rejects_large_objects_and_aggregate_before_payload_loading() {
        let temp = tempfile::tempdir().unwrap();
        git(Some(temp.path()), ["init", "--bare"], None).unwrap();
        let large = git(
            Some(temp.path()),
            ["hash-object", "-w", "--stdin"],
            Some(vec![b'x'; 16 * 1024 * 1024 + 1]),
        )
        .unwrap();
        let oid = String::from_utf8(large).unwrap().trim().to_owned();
        assert!(batch(temp.path(), &[oid])
            .unwrap_err()
            .message
            .contains("16 MiB"));
        let small = git(
            Some(temp.path()),
            ["hash-object", "-w", "--stdin"],
            Some(vec![b'x'; 4 * 1024 * 1024]),
        )
        .unwrap();
        let oid = String::from_utf8(small).unwrap().trim().to_owned();
        assert!(batch(temp.path(), &vec![oid; 17])
            .unwrap_err()
            .message
            .contains("64 MiB"));
    }
}
