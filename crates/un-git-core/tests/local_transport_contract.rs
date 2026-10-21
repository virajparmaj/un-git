//! Phase-0 Git transport experiments. There is deliberately no product push API.
//! Every receiver and object comes from a freshly generated, temporary fixture.
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
};
use un_git_core::engine::{self, Selection};

fn command(repo: &Path, args: &[&str], input: Option<&[u8]>) -> Output {
    let mut child = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .env_clear()
        .env("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "Transport fixture")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.test")
        .env("GIT_COMMITTER_NAME", "Transport fixture")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.test")
        .env("GIT_AUTHOR_DATE", "1609900000 +0000")
        .env("GIT_COMMITTER_DATE", "1609900000 +0000")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    child.wait_with_output().unwrap()
}

fn git(repo: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    let output = command(repo, args, input);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().into()
}

struct Fixture {
    _root: tempfile::TempDir,
    source: PathBuf,
    candidate: PathBuf,
    receiver: PathBuf,
    old: String,
    new: String,
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source.git");
        engine::create_demo(&source).unwrap();
        let scan = engine::scan(&source).unwrap();
        let old = scan.refs["refs/heads/main"].clone();
        let selected = scan
            .commits
            .iter()
            .find(|c| c.message.starts_with("Selected historical"))
            .unwrap();
        let plan = engine::plan(
            &scan,
            &Selection {
                commit_oids: vec![selected.oid.clone()],
                confirmed_email: "demo@un-git.test".into(),
                replacement_email: engine::default_replacement_email(),
            },
        )
        .unwrap();
        let prepared = engine::prepare(&plan, &root.path().join("operation")).unwrap();
        assert!(prepared.validation.passed);
        let receiver = root.path().join("receiver.git");
        git(
            root.path(),
            &[
                "clone",
                "--bare",
                "--no-local",
                "--",
                source.to_str().unwrap(),
                receiver.to_str().unwrap(),
            ],
            None,
        );
        let new = prepared.target_refs["refs/heads/main"].clone();
        Self {
            _root: root,
            source,
            receiver,
            candidate: prepared.candidate_path.into(),
            old,
            new,
        }
    }
    fn push(&self, from: &Path, expected: &str, target: &str) -> Output {
        // Exact lease and explicit object-to-ref mapping; never retry with force.
        command(
            from,
            &[
                "-c",
                "push.followTags=false",
                "-c",
                "push.default=nothing",
                "push",
                "--porcelain",
                "--atomic",
                &format!("--force-with-lease=refs/heads/main:{expected}"),
                "--",
                self.receiver.to_str().unwrap(),
                &format!("{target}:refs/heads/main"),
            ],
            None,
        )
    }
    fn tip(&self) -> String {
        git(&self.receiver, &["rev-parse", "refs/heads/main"], None)
    }
    fn concurrent_commit(&self) -> String {
        let parent = self.tip();
        let tree = git(
            &self.receiver,
            &["rev-parse", &format!("{parent}^{{tree}}")],
            None,
        );
        let next = git(
            &self.receiver,
            &["commit-tree", &tree, "-p", &parent],
            Some(b"Concurrent work\n"),
        );
        git(
            &self.receiver,
            &["update-ref", "refs/heads/main", &next, &parent],
            None,
        );
        next
    }
}

#[test]
fn exact_atomic_publication_and_reviewed_lease_restore_preserve_other_refs() {
    let fixture = Fixture::new();
    git(
        &fixture.receiver,
        &["update-ref", "refs/heads/unrelated", &fixture.old],
        None,
    );
    git(
        &fixture.candidate,
        &["tag", "must-not-push", &fixture.new],
        None,
    );
    assert!(fixture
        .push(&fixture.candidate, &fixture.old, &fixture.new)
        .status
        .success());
    assert_eq!(fixture.tip(), fixture.new);
    assert_eq!(
        git(
            &fixture.receiver,
            &["rev-parse", "refs/heads/unrelated"],
            None
        ),
        fixture.old
    );
    assert_eq!(git(&fixture.receiver, &["tag", "--list"], None), "");
    // A restore is a separate exact lease against the published candidate.
    assert!(fixture
        .push(&fixture.source, &fixture.new, &fixture.old)
        .status
        .success());
    assert_eq!(fixture.tip(), fixture.old);
}

#[test]
fn stale_publish_lease_preserves_concurrent_work() {
    let fixture = Fixture::new();
    let concurrent = fixture.concurrent_commit();
    assert!(!fixture
        .push(&fixture.candidate, &fixture.old, &fixture.new)
        .status
        .success());
    assert_eq!(fixture.tip(), concurrent);
}

#[test]
fn lost_acknowledgement_is_resolved_by_refs_and_restore_rejects_drift() {
    let fixture = Fixture::new();
    // Discard the command response to simulate a caller that lost its acknowledgement.
    drop(fixture.push(&fixture.candidate, &fixture.old, &fixture.new));
    assert_eq!(
        fixture.tip(),
        fixture.new,
        "Reconciliation must inspect refs, not infer failure"
    );
    let concurrent = fixture.concurrent_commit();
    assert!(!fixture
        .push(&fixture.source, &fixture.new, &fixture.old)
        .status
        .success());
    assert_eq!(fixture.tip(), concurrent);
}

#[test]
fn missing_atomic_capability_blocks_without_fallback() {
    let fixture = Fixture::new();
    git(
        &fixture.receiver,
        &["config", "receive.advertiseAtomic", "false"],
        None,
    );
    let output = fixture.push(&fixture.candidate, &fixture.old, &fixture.new);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("atomic"));
    assert_eq!(fixture.tip(), fixture.old);
}

#[test]
fn receiver_rejection_retains_old_tip_and_validated_candidate() {
    let fixture = Fixture::new();
    let hooks = fixture.receiver.join("hooks");
    fs::create_dir_all(&hooks).unwrap();
    let hook = hooks.join("pre-receive");
    fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o700)).unwrap();
    let output = fixture.push(&fixture.candidate, &fixture.old, &fixture.new);
    assert!(!output.status.success());
    assert_eq!(fixture.tip(), fixture.old);
    assert_eq!(
        git(&fixture.candidate, &["rev-parse", "refs/heads/main"], None),
        fixture.new
    );
}
