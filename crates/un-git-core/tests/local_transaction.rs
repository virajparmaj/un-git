use std::{
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};
use un_git_core::{
    engine::{self, Selection},
    operations::Application,
};

fn git(path: &Path, args: &[&str], input: Option<&[u8]>) -> String {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(path)
        .args(args)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        child.stdin.take().unwrap().write_all(input).unwrap();
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().into()
}

fn selection(scan: &engine::Scan) -> Selection {
    Selection {
        commit_oids: vec![scan
            .commits
            .iter()
            .find(|c| c.message.starts_with("Selected historical"))
            .unwrap()
            .oid
            .clone()],
        confirmed_email: "demo@un-git.test".into(),
        replacement_email: engine::default_replacement_email(),
    }
}

#[test]
fn real_local_transaction_preserves_history_and_is_deterministic() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source with spaces.git");
    engine::create_demo(&source).unwrap();
    let scan = engine::scan(&source).unwrap();
    assert_eq!(scan.commits.len(), 6);
    assert!(scan.blockers.is_empty(), "{:?}", scan.blockers);
    let plan = engine::plan(&scan, &selection(&scan)).unwrap();
    assert_eq!(plan.selected_oids.len(), 1);
    assert_eq!(plan.affected_oids.len(), 5);
    let mut states = Vec::new();
    let first = engine::prepare_with_progress(&plan, &temp.path().join("op-one"), |state| {
        states.push(state.to_string());
        Ok(())
    })
    .unwrap();
    assert_eq!(states, vec!["BACKUP_VERIFIED", "PREPARING", "VALIDATED"]);
    assert!(first.validation.passed, "{:?}", first.validation);
    let second = engine::prepare(&plan, &temp.path().join("op-two")).unwrap();
    assert_eq!(first.target_refs, second.target_refs);
    assert_eq!(first.commit_map, second.commit_map);
    assert_eq!(
        scan.id,
        engine::scan(&source).unwrap().id,
        "source was modified"
    );
    assert_eq!(first.commit_map.len(), 6);
    let root = scan.commits.iter().find(|c| c.parents.is_empty()).unwrap();
    assert_eq!(first.commit_map[&root.oid], root.oid);
    let mut tampered = first.clone();
    tampered
        .commit_map
        .insert(plan.selected_oids[0].clone(), root.oid.clone());
    assert!(
        !engine::verify(&plan, &tampered).unwrap().passed,
        "colliding mapping accepted"
    );
    fs::write(&second.backup_path, b"truncated bundle").unwrap();
    assert!(
        !engine::verify(&plan, &second).unwrap().passed,
        "corrupt bundle accepted"
    );
}

#[test]
fn stale_plan_tampering_and_unconfirmed_identity_are_blocked() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.git");
    engine::create_demo(&source).unwrap();
    let scan = engine::scan(&source).unwrap();
    let mut select = selection(&scan);
    select.confirmed_email = "other@example.test".into();
    assert!(engine::plan(&scan, &select).is_err());
    select = selection(&scan);
    select.commit_oids.push(select.commit_oids[0].clone());
    assert!(engine::plan(&scan, &select).is_err());
    select = selection(&scan);
    select.replacement_email = "--upload-pack=evil".into();
    assert!(engine::plan(&scan, &select).is_err());
    let mut plan = engine::plan(&scan, &selection(&scan)).unwrap();
    plan.affected_oids.clear();
    assert!(engine::prepare(&plan, &temp.path().join("tampered")).is_err());
    let plan = engine::plan(&scan, &selection(&scan)).unwrap();
    git(
        &source,
        &[
            "update-ref",
            "refs/heads/new-branch",
            &scan.refs["refs/heads/main"],
        ],
        None,
    );
    assert!(engine::prepare(&plan, &temp.path().join("stale")).is_err());
    assert!(!engine::scan(&source).unwrap().blockers.is_empty());
}

#[test]
fn application_journals_and_redacts_real_transaction() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::open(&temp.path().join("app")).unwrap();
    let demo = app.create_demo().unwrap();
    let scan = app.scan(&demo).unwrap();
    let plan = app.plan(&scan.id, &selection(&scan)).unwrap();
    let operation = app.prepare(&plan.id, |_| {}).unwrap();
    assert_eq!(operation.state, "VALIDATED");
    let states: Vec<_> = app
        .store
        .events(&operation.id)
        .unwrap()
        .into_iter()
        .map(|e| e.state)
        .collect();
    assert_eq!(
        states,
        vec!["PLANNED", "BACKUP_VERIFIED", "PREPARING", "VALIDATED"]
    );
    assert!(app.export_report(&operation.id).is_err());
    let preview = app.report_preview(&operation.id).unwrap();
    let report = preview.to_string();
    for secret in [
        &scan.source_path,
        "demo@un-git.test",
        &plan.selected_oids[0],
        "Selected historical commit",
    ] {
        assert!(!report.contains(secret));
    }
    let path = app.export_report(&operation.id).unwrap();
    assert!(path.is_file());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap(),
        preview
    );
    assert_eq!(app.verify(&operation.id).unwrap().state, "VALIDATED");
    fs::write(operation.prepared.unwrap().backup_path, b"broken").unwrap();
    assert_eq!(app.verify(&operation.id).unwrap().state, "BLOCKED");
}

#[test]
fn resumed_artifacts_cannot_use_external_links_or_alternates() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.git");
    engine::create_demo(&source).unwrap();
    let scan = engine::scan(&source).unwrap();
    let plan = engine::plan(&scan, &selection(&scan)).unwrap();
    let prepared = engine::prepare(&plan, &temp.path().join("op")).unwrap();
    let nested = Path::new(&prepared.candidate_path).join("objects/external");
    std::os::unix::fs::symlink(source.join("objects"), &nested).unwrap();
    assert!(engine::verify(&plan, &prepared).is_err());
    fs::remove_file(&nested).unwrap();
    fs::write(
        Path::new(&prepared.candidate_path).join("objects/info/alternates"),
        source.join("objects").to_str().unwrap(),
    )
    .unwrap();
    assert!(engine::verify(&plan, &prepared).is_err());
}

#[test]
fn local_revalidation_cannot_clear_a_remote_unknown_state() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::open(temp.path()).unwrap();
    let source = app.create_demo().unwrap();
    let scan = app.scan(&source).unwrap();
    let plan = app.plan(&scan.id, &selection(&scan)).unwrap();
    let mut operation = app.prepare(&plan.id, |_| {}).unwrap();
    operation.state = "REMOTE_OUTCOME_UNKNOWN".into();
    app.store
        .checkpoint(&operation.id, &operation.state, &operation)
        .unwrap();
    assert!(app.verify(&operation.id).is_err());
    assert_eq!(
        app.operation(&operation.id).unwrap().state,
        "REMOTE_OUTCOME_UNKNOWN"
    );
    assert!(app.prepare(&plan.id, |_| {}).is_err());
    operation.state = "VALIDATED".into();
    operation.prepared.as_mut().unwrap().operation_dir =
        temp.path().join("other/artifacts").to_string_lossy().into();
    app.store
        .checkpoint(&operation.id, &operation.state, &operation)
        .unwrap();
    assert!(app.verify(&operation.id).is_err());
    assert_eq!(app.operation(&operation.id).unwrap().state, "BLOCKED");
}

#[test]
fn unsupported_signature_and_non_utf8_messages_are_not_silently_normalized() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source.git");
    engine::create_demo(&source).unwrap();
    let scan = engine::scan(&source).unwrap();
    let tip = &scan.refs["refs/heads/main"];
    let tree = git(&source, &["rev-parse", &format!("{tip}^{{tree}}")], None);
    let mut raw=format!("tree {tree}\nparent {tip}\nauthor Demo Developer <demo@un-git.test> 1609804900 -0600\ncommitter Other <other@example.test> 1609804900 -0600\n\nRaw message: ").into_bytes();
    raw.extend_from_slice(&[0xff, 0xfe, b'\n']);
    let oid = git(
        &source,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(&raw),
    );
    git(&source, &["update-ref", "refs/heads/main", &oid], None);
    let scan = engine::scan(&source).unwrap();
    let plan = engine::plan(
        &scan,
        &Selection {
            commit_oids: vec![oid.clone()],
            confirmed_email: "demo@un-git.test".into(),
            replacement_email: engine::default_replacement_email(),
        },
    )
    .unwrap();
    assert!(
        engine::prepare(&plan, &temp.path().join("raw"))
            .unwrap()
            .validation
            .passed
    );
    let raw=format!("tree {tree}\nparent {oid}\nauthor Demo <demo@un-git.test> 1609805000 +0000\ncommitter Demo <demo@un-git.test> 1609805000 +0000\ngpgsig fake-signature\n continuation\n\nSigned-like commit\n");
    let signed = git(
        &source,
        &["hash-object", "-t", "commit", "-w", "--stdin"],
        Some(raw.as_bytes()),
    );
    git(&source, &["update-ref", "refs/heads/main", &signed], None);
    let scan = engine::scan(&source).unwrap();
    assert!(scan.blockers.iter().any(|b| b.contains("signatures")));
}

#[test]
fn cancellation_retains_verified_backup_and_never_rewrites_source() {
    let temp = tempfile::tempdir().unwrap();
    let app = Application::open(&temp.path().join("app")).unwrap();
    let source = app.create_demo().unwrap();
    let scan = app.scan(&source).unwrap();
    let plan = app.plan(&scan.id, &selection(&scan)).unwrap();
    let error = app
        .prepare(&plan.id, |operation| {
            if operation.state == "BACKUP_VERIFIED" {
                app.cancel(&operation.id).unwrap();
            }
        })
        .unwrap_err();
    assert_eq!(error.code, "CANCELLED_BEFORE_PUSH");
    let operation = app.operations().unwrap().remove(0);
    assert_eq!(operation.state, "CANCELLED_BEFORE_PUSH");
    assert!(app
        .store
        .operation_dir(&operation.id)
        .unwrap()
        .join("artifacts/backup.bundle")
        .is_file());
    assert_eq!(scan.id, engine::scan(&source).unwrap().id);
}
