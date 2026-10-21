//! Attribution rewriting must retain the file changes introduced by selected commits.
//! Build and inspect bare Git objects only; no user repository or checkout is involved.
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use un_git_core::engine::{self, Selection};

type Files = BTreeMap<String, Vec<u8>>;

fn git_bytes(repo: &Path, args: &[&str], input: Option<&[u8]>) -> Vec<u8> {
    let mut child = Command::new("git")
        .args(["-c", "core.hooksPath=/dev/null"])
        .args(["-c", "core.attributesFile=/dev/null"])
        .arg("-C")
        .arg(repo)
        .args(args)
        .env_clear()
        .env("PATH", "/opt/homebrew/bin:/usr/local/bin:/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    if let Some(bytes) = input {
        child.stdin.take().unwrap().write_all(bytes).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

fn git_text(repo: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    String::from_utf8(git_bytes(repo, args, input))
        .unwrap()
        .trim()
        .into()
}

fn write_tree(repo: &Path, files: &Files) -> String {
    let mut directories: BTreeMap<String, Files> = BTreeMap::new();
    let mut entries = String::new();
    for (path, bytes) in files {
        if let Some((directory, relative)) = path.split_once('/') {
            directories
                .entry(directory.into())
                .or_default()
                .insert(relative.into(), bytes.clone());
        } else {
            let oid = git_text(repo, &["hash-object", "-w", "--stdin"], Some(bytes));
            entries.push_str(&format!("100644 blob {oid}\t{path}\n"));
        }
    }
    for (directory, files) in directories {
        let oid = write_tree(repo, &files);
        entries.push_str(&format!("040000 tree {oid}\t{directory}\n"));
    }
    git_text(repo, &["mktree"], Some(entries.as_bytes()))
}

fn write_commit(repo: &Path, files: &Files, parent: Option<&str>, email: &str) -> String {
    let tree = write_tree(repo, files);
    let parent_header = parent.map_or_else(String::new, |oid| format!("parent {oid}\n"));
    let raw = format!(
        "tree {tree}\n{parent_header}\
         author Content Fixture <{email}> 1609545600 -0600\n\
         committer Fixture Maintainer <maintainer@example.test> 1609549200 +0530\n\n\
         Keep the changes in this commit.\n"
    );
    git_text(
        repo,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(raw.as_bytes()),
    )
}

fn read_files(repo: &Path, commit: &str) -> Files {
    let listing = git_bytes(repo, &["ls-tree", "-r", "-z", "--full-tree", commit], None);
    listing
        .split(|byte| *byte == 0)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            let entry = std::str::from_utf8(entry).unwrap();
            let (metadata, path) = entry.split_once('\t').unwrap();
            let mut fields = metadata.split_whitespace();
            assert_eq!(fields.next(), Some("100644"));
            assert_eq!(fields.next(), Some("blob"));
            let oid = fields.next().unwrap();
            (
                path.into(),
                git_bytes(repo, &["cat-file", "blob", oid], None),
            )
        })
        .collect()
}

#[test]
fn selected_middle_commit_keeps_all_file_changes_and_historical_snapshots() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("content fixture.git");
    fs::create_dir(&source).unwrap();
    git_text(
        &source,
        &["init", "--bare", "--initial-branch=main", "--template="],
        None,
    );

    let root_files = Files::from([
        ("README.md".into(), b"Original documentation.\n".to_vec()),
        (
            "obsolete.txt".into(),
            b"Removed by the selected commit.\n".to_vec(),
        ),
        ("assets/sample.bin".into(), vec![0, 255, 254, 1, 13, 10]),
    ]);
    let root = write_commit(&source, &root_files, None, "other@example.test");

    let feature = b"This feature was added by the selected commit and must remain.\n";
    let binary = vec![0, 128, 255, 0, 10, 13, 254, 42];
    let mut selected_files = root_files.clone();
    selected_files.insert("README.md".into(), b"Feature documented.\n".to_vec());
    selected_files.insert("src/feature.txt".into(), feature.to_vec());
    selected_files.insert("assets/sample.bin".into(), binary.clone());
    selected_files.remove("obsolete.txt");
    let selected = write_commit(
        &source,
        &selected_files,
        Some(&root),
        "selected@example.test",
    );

    let mut final_files = selected_files.clone();
    final_files.insert(
        "README.md".into(),
        b"Feature documented and extended.\n".to_vec(),
    );
    final_files.insert("later.txt".into(), b"A later author's addition.\n".to_vec());
    let tip = write_commit(&source, &final_files, Some(&selected), "other@example.test");
    git_text(&source, &["update-ref", "refs/heads/main", &tip], None);

    let scan = engine::scan(&source).unwrap();
    assert!(scan.blockers.is_empty(), "{:?}", scan.blockers);
    let plan = engine::plan(
        &scan,
        &Selection {
            commit_oids: vec![selected.clone()],
            confirmed_email: "selected@example.test".into(),
            replacement_email: engine::default_replacement_email(),
        },
    )
    .unwrap();
    assert_eq!(plan.selected_oids, vec![selected.clone()]);
    assert_eq!(plan.affected_oids.len(), 2);
    let prepared = engine::prepare(&plan, &temp.path().join("operation")).unwrap();
    assert!(prepared.validation.passed, "{:?}", prepared.validation);
    let candidate = Path::new(&prepared.candidate_path);
    let restored = Path::new(&prepared.restored_path);
    assert_eq!(prepared.commit_map.len(), 3);
    assert_eq!(prepared.commit_map[&root], root);
    assert_ne!(prepared.commit_map[&selected], selected);
    assert_ne!(prepared.commit_map[&tip], tip);

    for (old, expected_files) in [
        (&root, &root_files),
        (&selected, &selected_files),
        (&tip, &final_files),
    ] {
        let new = &prepared.commit_map[old];
        let original_tree = git_text(&source, &["rev-parse", &format!("{old}^{{tree}}")], None);
        assert_eq!(
            git_text(candidate, &["rev-parse", &format!("{new}^{{tree}}")], None),
            original_tree,
            "A mapped commit changed its file tree"
        );
        assert_eq!(read_files(&source, old), *expected_files);
        assert_eq!(read_files(candidate, new), *expected_files);
        assert_eq!(read_files(restored, old), *expected_files);
    }

    let mapped_selected = &prepared.commit_map[&selected];
    let mapped_tip = &prepared.commit_map[&tip];
    assert_eq!(
        git_text(
            candidate,
            &["show", "-s", "--format=%ae", mapped_selected],
            None
        ),
        engine::default_replacement_email()
    );
    // Compare the actual add/modify/delete deltas, not merely the final tip.
    for (old_parent, old_commit) in [(&root, &selected), (&selected, &tip)] {
        assert_eq!(
            git_bytes(
                &source,
                &[
                    "diff-tree",
                    "--no-commit-id",
                    "-r",
                    "--raw",
                    old_parent,
                    old_commit
                ],
                None
            ),
            git_bytes(
                candidate,
                &[
                    "diff-tree",
                    "--no-commit-id",
                    "-r",
                    "--raw",
                    &prepared.commit_map[old_parent],
                    &prepared.commit_map[old_commit]
                ],
                None
            )
        );
    }
    let actual_final = read_files(candidate, "refs/heads/main");
    assert_eq!(actual_final, final_files);
    assert_eq!(actual_final["src/feature.txt"], feature);
    assert_eq!(actual_final["assets/sample.bin"], binary);
    assert!(!actual_final.contains_key("obsolete.txt"));
    assert_eq!(prepared.target_refs["refs/heads/main"], *mapped_tip);
    assert_eq!(engine::scan(&source).unwrap().id, scan.id, "Source changed");
}
