use super::runner::*;
use crate::{CoreError, Result};
use std::path::Path;
use std::process::Command;

fn commit(
    path: &Path,
    tree: &str,
    parents: &[&str],
    name: &str,
    email: &str,
    timestamp: &str,
    message: &[u8],
) -> Result<String> {
    let mut command = Command::new(git_executable()?);
    isolated(&mut command);
    command.arg("-C").arg(path).arg("commit-tree").arg(tree);
    for parent in parents {
        command.arg("-p").arg(parent);
    }
    command
        .env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_AUTHOR_DATE", timestamp)
        .env("GIT_COMMITTER_NAME", "Fixture Maintainer")
        .env("GIT_COMMITTER_EMAIL", "maintainer@example.test")
        .env("GIT_COMMITTER_DATE", timestamp);
    Ok(
        String::from_utf8_lossy(&execute(command, Some(message.to_vec()))?)
            .trim()
            .into(),
    )
}

pub fn create_demo(path: &Path) -> Result<()> {
    if path.exists() {
        return Err(CoreError::invalid(
            "Demo destination must not already exist",
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    git(
        None,
        [
            std::ffi::OsStr::new("init"),
            std::ffi::OsStr::new("--bare"),
            std::ffi::OsStr::new("--initial-branch=main"),
            std::ffi::OsStr::new("--"),
            path.as_os_str(),
        ],
        None,
    )?;
    let path = path.canonicalize()?;
    let blob = String::from_utf8_lossy(&git(
        Some(&path),
        ["hash-object", "-w", "--stdin"],
        Some(b"unchanged source content\n".to_vec()),
    )?)
    .trim()
    .to_string();
    let tree = String::from_utf8_lossy(&git(
        Some(&path),
        ["mktree"],
        Some(format!("100644 blob {blob}\tREADME.txt\n").into_bytes()),
    )?)
    .trim()
    .to_string();
    let root = commit(
        &path,
        &tree,
        &[],
        "Other Author",
        "other@example.test",
        "1609459200 +0000",
        b"Root from another identity\n",
    )?;
    let selected = commit(
        &path,
        &tree,
        &[&root],
        "Demo Developer",
        "demo@un-git.test",
        "1609545600 -0600",
        b"Selected historical commit\n\nCo-authored-by: Other Author <other@example.test>\n",
    )?;
    let left = commit(
        &path,
        &tree,
        &[&selected],
        "Other Author",
        "other@example.test",
        "1609632000 +0530",
        b"Later descendant\n",
    )?;
    let side = commit(
        &path,
        &tree,
        &[&selected],
        "Other Author",
        "other@example.test",
        "1609635600 -0800",
        b"Side of preserved merge\n",
    )?;
    let merge = commit(
        &path,
        &tree,
        &[&left, &side],
        "Other Author",
        "other@example.test",
        "1609718400 +0000",
        format!("Merge; preserve message reference {selected}\n").as_bytes(),
    )?;
    let empty = commit(
        &path,
        &tree,
        &[&merge],
        "Demo Developer",
        "demo@un-git.test",
        "1609804800 +0000",
        b"Intentionally empty commit\n",
    )?;
    git(Some(&path), ["update-ref", "refs/heads/main", &empty], None)?;
    Ok(())
}
