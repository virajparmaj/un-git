use super::{model::*, runner::*, scan};
use crate::{CoreError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::fs::MetadataExt;
use std::path::Path;

fn check(checks: &mut Vec<ValidationCheck>, name: &str, passed: bool, detail: &str) {
    checks.push(ValidationCheck {
        name: name.into(),
        passed,
        detail: detail.into(),
    });
}

fn expected_payload(
    plan: &Plan,
    oid: &str,
    raw: &[u8],
    mapping: &BTreeMap<String, String>,
) -> Result<Vec<u8>> {
    let (headers, message) = scan::split_raw(raw)?;
    let mut expected = Vec::new();
    for line in headers.split(|b| *b == b'\n') {
        if let Some(parent) = line.strip_prefix(b"parent ") {
            let parent = std::str::from_utf8(parent)
                .map_err(|_| CoreError::invalid("Invalid parent OID"))?;
            let mapped = mapping
                .get(parent)
                .ok_or_else(|| CoreError::blocked("Missing parent mapping"))?;
            expected.extend_from_slice(b"parent ");
            expected.extend_from_slice(mapped.as_bytes());
        } else if plan.selected_oids.iter().any(|selected| selected == oid)
            && line.starts_with(b"author ")
        {
            let identity = &line[b"author ".len()..];
            let (_, email, _) = scan::identity(identity)?;
            if email != plan.selection.confirmed_email.as_bytes() {
                return Err(CoreError::blocked(
                    "Selected raw author differs from confirmed identity",
                ));
            }
            let open = line
                .iter()
                .rposition(|b| *b == b'<')
                .ok_or_else(|| CoreError::invalid("Invalid author"))?;
            let close = line
                .iter()
                .rposition(|b| *b == b'>')
                .ok_or_else(|| CoreError::invalid("Invalid author"))?;
            expected.extend_from_slice(&line[..open + 1]);
            expected.extend_from_slice(plan.selection.replacement_email.as_bytes());
            expected.extend_from_slice(&line[close..]);
        } else {
            expected.extend_from_slice(line);
        }
        expected.push(b'\n');
    }
    expected.push(b'\n');
    expected.extend_from_slice(message);
    Ok(expected)
}

pub fn verify(plan: &Plan, prepared: &PreparedCandidate) -> Result<ValidationReport> {
    if std::fs::symlink_metadata(&prepared.operation_dir)?
        .file_type()
        .is_symlink()
    {
        return Err(CoreError::invalid(
            "Operation directory cannot be a symbolic link",
        ));
    }
    let operation = Path::new(&prepared.operation_dir).canonicalize()?;
    let candidate = Path::new(&prepared.candidate_path);
    let restored = Path::new(&prepared.restored_path);
    let backup = Path::new(&prepared.backup_path);
    for (path, basename) in [
        (candidate, "candidate.git"),
        (restored, "restored.git"),
        (backup, "backup.bundle"),
    ] {
        if path != operation.join(basename)
            || std::fs::symlink_metadata(path)?.file_type().is_symlink()
        {
            return Err(CoreError::invalid(
                "Prepared artifact paths escaped the operation directory",
            ));
        }
        inspect_artifact_tree(path)?;
    }
    for repository in [candidate, restored] {
        for relative in [
            "objects/info/alternates",
            "objects/info/http-alternates",
            "info/grafts",
            "shallow",
        ] {
            if repository.join(relative).exists() {
                return Err(CoreError::blocked(
                    "Prepared repositories must retain complete independent object storage",
                ));
            }
        }
    }
    let mut checks = Vec::new();
    let valid_plan = plan.version == SCHEMA_VERSION
        && plan.id == plan.content_hash
        && plan.blockers.is_empty()
        && scan::hash_plan(plan)? == plan.content_hash
        && scan::plan(&plan.scan, &plan.selection)?.content_hash == plan.content_hash;
    check(
        &mut checks,
        "plan_integrity",
        valid_plan,
        "Versioned plan, selection, and impact are bound to the content hash",
    );
    let backup_ok = file_digest(backup)? == prepared.backup_sha256;
    check(
        &mut checks,
        "backup_checksum",
        backup_ok,
        "Standalone bundle bytes match their recorded SHA-256",
    );
    let restored_objects = scan::raw_commits(restored)?;
    let candidate_objects = scan::raw_commits(candidate)?;
    let baseline_objects: BTreeMap<String, Vec<u8>> = plan
        .scan
        .commits
        .iter()
        .map(|commit| {
            STANDARD
                .decode(&commit.raw_base64)
                .map(|raw| (commit.oid.clone(), raw))
                .map_err(|_| CoreError::invalid("Invalid baseline raw payload"))
        })
        .collect::<Result<_>>()?;
    let restored_ok =
        restored_objects == baseline_objects && scan::refs(restored)? == plan.scan.refs;
    check(
        &mut checks,
        "backup_restoration",
        restored_ok,
        "Independent restored refs and every reachable commit reproduce the baseline",
    );
    let originals: BTreeSet<_> = baseline_objects.keys().cloned().collect();
    let mapping_keys: BTreeSet<_> = prepared.commit_map.keys().cloned().collect();
    let mapping_values: BTreeSet<_> = prepared.commit_map.values().cloned().collect();
    let candidate_ids: BTreeSet<_> = candidate_objects.keys().cloned().collect();
    let mapping_ok = originals == mapping_keys
        && prepared.commit_map.len() == mapping_values.len()
        && mapping_values == candidate_ids;
    check(&mut checks, "complete_injective_mapping", mapping_ok, "One nonzero target per original, no collisions, missing entries, or extra reachable commits");
    let candidate_refs = scan::refs(candidate)?;
    let expected_refs: BTreeMap<_, _> = plan
        .scan
        .refs
        .iter()
        .map(|(name, old)| {
            (
                name.clone(),
                prepared.commit_map.get(old).cloned().unwrap_or_default(),
            )
        })
        .collect();
    check(
        &mut checks,
        "exact_ref_scope",
        candidate_refs == expected_refs && candidate_refs == prepared.target_refs,
        "Only the planned ref set exists, with mapped target OIDs",
    );
    let mut trees = true;
    let mut topology = true;
    let mut dates = true;
    let mut messages = true;
    let mut identities = true;
    let mut raw_bytes = true;
    let mut boundary = true;
    for (oid, old_raw) in &baseline_objects {
        let Some(mapped) = prepared.commit_map.get(oid) else {
            trees = false;
            topology = false;
            raw_bytes = false;
            continue;
        };
        let Some(new_raw) = candidate_objects.get(mapped) else {
            trees = false;
            topology = false;
            raw_bytes = false;
            continue;
        };
        let old = scan::parse_commit(oid.clone(), old_raw.clone())?;
        let new = scan::parse_commit(mapped.clone(), new_raw.clone())?;
        trees &= old.tree_oid == new.tree_oid;
        let expected_parents: Vec<_> = old
            .parents
            .iter()
            .map(|p| prepared.commit_map.get(p).cloned().unwrap_or_default())
            .collect();
        topology &= new.parents == expected_parents;
        dates &= old.author_date == new.author_date && old.committer_date == new.committer_date;
        messages &= scan::split_raw(old_raw)?.1 == scan::split_raw(new_raw)?.1;
        let expected_email = if plan.selected_oids.contains(oid) {
            &plan.selection.replacement_email
        } else {
            &old.author_email
        };
        identities &= old.author_name == new.author_name
            && new.author_email == *expected_email
            && old.committer_name == new.committer_name
            && old.committer_email == new.committer_email;
        raw_bytes &= expected_payload(plan, oid, old_raw, &prepared.commit_map)
            .is_ok_and(|expected| expected == *new_raw);
        boundary &= plan.affected_oids.contains(oid) || oid == mapped;
    }
    for (name, result, detail) in [
        ("tree_equality", trees, "Every mapped commit preserves the exact tree OID"),
        ("ordered_parent_topology", topology, "Ordered parent edges equal the independently mapped baseline"),
        ("raw_dates", dates, "Author and committer timestamps and offsets are unchanged"),
        ("message_bytes", messages, "All message bytes, including coauthor trailers and SHA references, are unchanged"),
        ("explicit_identity_edits", identities, "Only the selected author email is edited; all names and committer identities remain unchanged"),
        ("complete_raw_payload", raw_bytes, "Raw object bytes differ only at approved author email spans and mapped parent OIDs"),
        ("descendant_change_boundary", boundary, "Every commit outside selected commits and descendants retains its original OID"),
    ] { check(&mut checks, name, result, detail); }
    let integrity = git_text(Some(candidate), &["fsck", "--full", "--strict"]).is_ok()
        && git_text(Some(restored), &["fsck", "--full", "--strict"]).is_ok();
    check(
        &mut checks,
        "object_connectivity",
        integrity,
        "Candidate and restored backup pass strict Git object/connectivity checks",
    );
    Ok(ValidationReport {
        passed: checks.iter().all(|c| c.passed),
        checks,
        baseline_commit_count: baseline_objects.len(),
        candidate_commit_count: candidate_objects.len(),
    })
}

fn inspect_artifact_tree(path: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || (!metadata.is_dir() && !metadata.is_file())
        || (metadata.is_file() && metadata.nlink() != 1)
    {
        return Err(CoreError::blocked(
            "Prepared artifacts must contain only independent regular files and directories",
        ));
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(path)? {
            inspect_artifact_tree(&entry?.path())?;
        }
    }
    Ok(())
}
