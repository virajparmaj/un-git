use super::{limits, model::*, runner::*, scan, validate};
use crate::{CoreError, Result};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::Command;

pub(crate) fn durable_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("json.pending");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temporary)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    fs::rename(&temporary, path)?;
    File::open(
        path.parent()
            .ok_or_else(|| CoreError::invalid("Missing artifact parent"))?,
    )?
    .sync_all()?;
    Ok(())
}

fn sync_tree(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() {
        return Err(CoreError::blocked(
            "Operation artifacts must not contain symbolic links",
        ));
    }
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for entry in fs::read_dir(path)? {
            sync_tree(&entry?.path())?;
        }
    } else {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    File::open(path)?.sync_all()?;
    Ok(())
}

fn clone_independent(source: &Path, destination: &Path) -> Result<()> {
    git(
        None,
        [
            std::ffi::OsStr::new("clone"),
            std::ffi::OsStr::new("--mirror"),
            std::ffi::OsStr::new("--no-local"),
            std::ffi::OsStr::new("--no-hardlinks"),
            std::ffi::OsStr::new("--"),
            source.as_os_str(),
            destination.as_os_str(),
        ],
        None,
    )?;
    Ok(())
}

fn validate_plan(plan: &Plan) -> Result<()> {
    if plan.scan.commits.len() > limits::MAX_PREPARE_COMMITS {
        return Err(CoreError::blocked(
            "The plan exceeds the preview preparation limit",
        ));
    }
    if plan.version != SCHEMA_VERSION
        || scan::hash_plan(plan)? != plan.content_hash
        || plan.id != plan.content_hash
    {
        return Err(CoreError::invalid("Plan schema or content hash mismatch"));
    }
    let rebuilt = scan::plan(&plan.scan, &plan.selection)?;
    if rebuilt.content_hash != plan.content_hash {
        return Err(CoreError::invalid("Plan impact or selection was modified"));
    }
    if !plan.blockers.is_empty() {
        return Err(CoreError::blocked(plan.blockers.join("; ")));
    }
    if git_text(None, &["--version"])? != plan.scan.toolchain.git_version {
        return Err(CoreError::blocked(
            "Git runtime changed since the plan; create a new plan",
        ));
    }
    if file_digest(&worker_dir().join("git_filter_repo.py"))? != FILTER_REPO_SHA256 {
        return Err(CoreError::blocked(
            "Pinned rewrite dependency checksum mismatch",
        ));
    }
    let expected_script = include_bytes!("../../../../workers/rewrite/worker.py");
    if plan.scan.toolchain.python_version != python_version()?
        || plan.scan.toolchain.callback_sha256 != digest(expected_script)
    {
        return Err(CoreError::blocked(
            "Python runtime or fixed callback changed since scanning; scan and plan again.",
        ));
    }
    if file_digest(&worker_dir().join("worker.py"))? != digest(expected_script) {
        return Err(CoreError::blocked(
            "Fixed rewrite callback changed; rebuild the application",
        ));
    }
    Ok(())
}

pub fn prepare(plan: &Plan, operation_dir: &Path) -> Result<PreparedCandidate> {
    prepare_with_progress(plan, operation_dir, |_| Ok(()))
}

pub fn prepare_with_progress(
    plan: &Plan,
    operation_dir: &Path,
    mut progress: impl FnMut(&str) -> Result<()>,
) -> Result<PreparedCandidate> {
    validate_plan(plan)?;
    let original = scan::scan(Path::new(&plan.scan.source_path))?;
    if original.id != plan.scan.id {
        return Err(CoreError::blocked(
            "Source history or eligibility changed; scan and plan again",
        ));
    }
    fs::create_dir_all(operation_dir)?;
    if fs::symlink_metadata(operation_dir)?
        .file_type()
        .is_symlink()
    {
        return Err(CoreError::blocked(
            "Operation directory must not be a symbolic link",
        ));
    }
    fs::set_permissions(operation_dir, fs::Permissions::from_mode(0o700))?;
    let operation_dir = operation_dir.canonicalize()?;
    if operation_dir.starts_with(Path::new(&plan.scan.source_path)) {
        return Err(CoreError::blocked(
            "Operation data must be outside the source repository",
        ));
    }
    for name in [
        "engine-plan.json",
        "capacity-check.json",
        "baseline.git",
        "backup.bundle",
        "backup-manifest.json",
        "restored.git",
        "candidate.git",
        "selection.json",
        "validation.json",
        "prepared.json",
    ] {
        if operation_dir.join(name).exists() {
            return Err(CoreError::blocked(
                "This operation already contains artifacts; reconcile it before preparing again",
            ));
        }
    }
    let (source_object_bytes, required_free_bytes) =
        limits::preparation_space(Path::new(&plan.scan.source_path), &operation_dir)?;
    durable_json(
        &operation_dir.join("capacity-check.json"),
        &serde_json::json!({
            "version": SCHEMA_VERSION, "source_object_bytes": source_object_bytes,
            "required_free_bytes": required_free_bytes, "checked_at": chrono::Utc::now().to_rfc3339()
        }),
    )?;
    durable_json(&operation_dir.join("engine-plan.json"), plan)?;
    let baseline = operation_dir.join("baseline.git");
    clone_independent(Path::new(&plan.scan.source_path), &baseline)?;
    if scan::refs(&baseline)? != plan.scan.refs
        || scan::raw_commits(&baseline)? != scan::raw_commits(Path::new(&plan.scan.source_path))?
    {
        return Err(CoreError::blocked(
            "Source moved while capturing the independent baseline",
        ));
    }
    if scan::refs(Path::new(&plan.scan.source_path))? != plan.scan.refs {
        return Err(CoreError::blocked("Source refs moved during capture"));
    }
    git_text(Some(&baseline), &["fsck", "--full", "--strict"])?;
    let backup = operation_dir.join("backup.bundle");
    git(
        Some(&baseline),
        [
            std::ffi::OsStr::new("bundle"),
            std::ffi::OsStr::new("create"),
            backup.as_os_str(),
            std::ffi::OsStr::new("--all"),
        ],
        None,
    )?;
    git(
        Some(&baseline),
        [
            std::ffi::OsStr::new("bundle"),
            std::ffi::OsStr::new("verify"),
            backup.as_os_str(),
        ],
        None,
    )?;
    let restored = operation_dir.join("restored.git");
    clone_independent(&backup, &restored)?;
    git_text(Some(&restored), &["fsck", "--full", "--strict"])?;
    if scan::refs(&restored)? != plan.scan.refs
        || scan::raw_commits(&restored)? != scan::raw_commits(&baseline)?
    {
        return Err(CoreError::blocked(
            "Independent backup restoration did not reproduce baseline objects and refs",
        ));
    }
    let backup_sha256 = file_digest(&backup)?;
    sync_tree(&backup)?;
    sync_tree(&restored)?;
    sync_tree(&baseline)?;
    durable_json(
        &operation_dir.join("backup-manifest.json"),
        &serde_json::json!({
            "version": SCHEMA_VERSION, "plan_hash": plan.content_hash,
            "backup_sha256": backup_sha256, "restored_refs": plan.scan.refs,
        }),
    )?;
    progress("BACKUP_VERIFIED")?;
    progress("PREPARING")?;
    limits::preparation_space(Path::new(&plan.scan.source_path), &operation_dir)?;
    let candidate = operation_dir.join("candidate.git");
    clone_independent(&restored, &candidate)?;
    let selection = operation_dir.join("selection.json");
    durable_json(
        &selection,
        &serde_json::json!({
            "version": SCHEMA_VERSION, "selected_oids": plan.selected_oids,
            "confirmed_email": plan.selection.confirmed_email, "replacement_email": plan.selection.replacement_email,
            "refs": plan.scan.refs.keys().collect::<Vec<_>>()
        }),
    )?;
    let mut command = Command::new(python_executable()?);
    isolated(&mut command);
    command
        .arg("-I")
        .arg(worker_dir().join("worker.py"))
        .arg(&candidate)
        .arg(&selection)
        .current_dir(&candidate);
    execute(command, None)?;
    let mut commit_map = parse_commit_map(&candidate.join("filter-repo/commit-map"))?;
    let candidate_objects = scan::raw_commits(&candidate)?;
    for commit in &plan.scan.commits {
        if !commit_map.contains_key(&commit.oid) && candidate_objects.contains_key(&commit.oid) {
            commit_map.insert(commit.oid.clone(), commit.oid.clone());
        }
    }
    let target_refs = scan::refs(&candidate)?;
    let mut prepared = PreparedCandidate {
        operation_dir: operation_dir.to_string_lossy().into(),
        backup_path: backup.to_string_lossy().into(),
        candidate_path: candidate.to_string_lossy().into(),
        restored_path: restored.to_string_lossy().into(),
        commit_map,
        target_refs,
        backup_sha256,
        validation: ValidationReport {
            passed: false,
            checks: Vec::new(),
            baseline_commit_count: plan.scan.commits.len(),
            candidate_commit_count: 0,
        },
    };
    prepared.validation = validate::verify(plan, &prepared)?;
    durable_json(&operation_dir.join("validation.json"), &prepared.validation)?;
    if !prepared.validation.passed {
        return Err(CoreError::blocked(
            "Candidate failed preservation validation; artifacts retained for inspection",
        ));
    }
    sync_tree(&candidate)?;
    durable_json(&operation_dir.join("prepared.json"), &prepared)?;
    progress("VALIDATED")?;
    Ok(prepared)
}

fn parse_commit_map(path: &Path) -> Result<BTreeMap<String, String>> {
    let text = fs::read_to_string(path)?;
    let mut result = BTreeMap::new();
    for line in text.lines().skip(1) {
        let fields: Vec<_> = line.split_whitespace().collect();
        if fields.len() != 2
            || !checked_oid(fields[0])
            || !checked_oid(fields[1])
            || fields[1].bytes().all(|b| b == b'0')
        {
            return Err(CoreError::blocked("Incomplete or invalid worker mapping"));
        }
        if result.insert(fields[0].into(), fields[1].into()).is_some() {
            return Err(CoreError::blocked("Duplicate worker mapping"));
        }
    }
    Ok(result)
}
