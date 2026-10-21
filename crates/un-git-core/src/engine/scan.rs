use super::limits::{MAX_PREPARE_COMMITS, MAX_REACHABLE_OBJECTS, MAX_SCAN_COMMITS};
use super::model::*;
use super::runner::*;
use crate::{CoreError, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

pub fn doctor() -> DoctorReport {
    let mut issues = Vec::new();
    let git_version = match git_text(None, &["--version"]) {
        Ok(version) => Some(version),
        Err(error) => {
            issues.push(error.message);
            None
        }
    };
    let python_version = python_version()
        .map_err(|error| issues.push(error.message))
        .ok();
    let worker_version = match file_digest(&worker_dir().join("git_filter_repo.py")) {
        Ok(hash) if hash == FILTER_REPO_SHA256 => Some(FILTER_REPO_VERSION.into()),
        _ => {
            issues.push("Pinned filter-repo source missing or checksum mismatch".into());
            None
        }
    };
    DoctorReport {
        ready: issues.is_empty(),
        git_version,
        python_version,
        worker_version,
        issues,
    }
}

pub(crate) fn refs(path: &Path) -> Result<BTreeMap<String, String>> {
    let output = git_text(
        Some(path),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    )?;
    let mut result = BTreeMap::new();
    for line in output.lines() {
        let (name, oid) = line
            .split_once(' ')
            .ok_or_else(|| CoreError::local("Malformed ref inventory"))?;
        if !name.starts_with("refs/") || !checked_oid(oid) {
            return Err(CoreError::blocked("Unsupported ref inventory"));
        }
        result.insert(name.into(), oid.into());
    }
    Ok(result)
}

pub(crate) fn raw_commits(path: &Path) -> Result<BTreeMap<String, Vec<u8>>> {
    let oids: Vec<String> = git_text(
        Some(path),
        &["rev-list", "--all", "--topo-order", "--reverse"],
    )?
    .lines()
    .map(str::to_owned)
    .collect();
    if oids.len() > MAX_SCAN_COMMITS {
        return Err(CoreError::blocked(format!("This preview can inspect at most {MAX_SCAN_COMMITS} reachable commits. No repository data was changed.")));
    }
    let payloads = batch(path, &oids)?;
    Ok(oids.into_iter().zip(payloads).collect())
}

pub(crate) fn split_raw(raw: &[u8]) -> Result<(&[u8], &[u8])> {
    let split = raw
        .windows(2)
        .position(|w| w == b"\n\n")
        .ok_or_else(|| CoreError::blocked("Malformed commit object"))?;
    Ok((&raw[..split], &raw[split + 2..]))
}

pub(crate) fn identity(line: &[u8]) -> Result<(&[u8], &[u8], &[u8])> {
    let open = line
        .iter()
        .rposition(|b| *b == b'<')
        .ok_or_else(|| CoreError::blocked("Malformed identity"))?;
    let close = line
        .iter()
        .rposition(|b| *b == b'>')
        .ok_or_else(|| CoreError::blocked("Malformed identity"))?;
    if open == 0
        || close <= open
        || line.get(open - 1) != Some(&b' ')
        || line.get(close + 1) != Some(&b' ')
    {
        return Err(CoreError::blocked("Malformed identity delimiters"));
    }
    Ok((
        &line[..open - 1],
        &line[open + 1..close],
        &line[close + 2..],
    ))
}

pub(crate) fn parse_commit(oid: String, raw: Vec<u8>) -> Result<Commit> {
    let (headers, message) = split_raw(&raw)?;
    let mut tree = String::new();
    let mut parents: Vec<String> = Vec::new();
    let mut author = None;
    let mut committer = None;
    let mut unsupported_headers = Vec::new();
    for line in headers.split(|b| *b == b'\n') {
        if let Some(value) = line.strip_prefix(b"tree ") {
            tree = String::from_utf8_lossy(value).into();
        } else if let Some(value) = line.strip_prefix(b"parent ") {
            parents.push(String::from_utf8_lossy(value).into());
        } else if let Some(value) = line.strip_prefix(b"author ") {
            if author.is_some() {
                unsupported_headers.push("duplicate-author".into());
            }
            author = Some(identity(value)?);
        } else if let Some(value) = line.strip_prefix(b"committer ") {
            if committer.is_some() {
                unsupported_headers.push("duplicate-committer".into());
            }
            committer = Some(identity(value)?);
        } else {
            let name = line.split(|b| *b == b' ').next().unwrap_or(b"continuation");
            unsupported_headers.push(String::from_utf8_lossy(name).into());
        }
    }
    let (an, ae, ad) = author.ok_or_else(|| CoreError::blocked("Commit has no author"))?;
    let (cn, ce, cd) = committer.ok_or_else(|| CoreError::blocked("Commit has no committer"))?;
    if !checked_oid(&tree) || parents.iter().any(|p| !checked_oid(p)) {
        return Err(CoreError::blocked("Invalid commit object references"));
    }
    Ok(Commit {
        oid,
        tree_oid: tree,
        parents,
        author_name: String::from_utf8_lossy(an).into(),
        author_email: String::from_utf8_lossy(ae).into(),
        author_date: String::from_utf8_lossy(ad).into(),
        committer_name: String::from_utf8_lossy(cn).into(),
        committer_email: String::from_utf8_lossy(ce).into(),
        committer_date: String::from_utf8_lossy(cd).into(),
        message: String::from_utf8_lossy(message).into(),
        raw_base64: STANDARD.encode(&raw),
        unsupported_headers,
    })
}

fn inspect_storage(path: &Path, format: &str, blockers: &mut Vec<String>) -> Result<()> {
    let git_dir = git_text(Some(path), &["rev-parse", "--absolute-git-dir"])?;
    for (relative, reason) in [
        ("shallow", "Shallow history"),
        ("info/grafts", "Grafts"),
        ("objects/info/alternates", "Alternate object storage"),
    ] {
        if Path::new(&git_dir).join(relative).exists() {
            blockers.push(format!("{reason} is inspection-only"));
        }
    }
    let config = git_text(Some(path), &["config", "--local", "--list"])?;
    if config
        .lines()
        .any(|line| line.contains(".promisor=") || line.starts_with("extensions.partialclone="))
    {
        blockers.push("Partial clone boundaries are inspection-only".into());
    }
    if format != "sha1" {
        blockers.push("Only the tested SHA-1 object format can be prepared".into());
    }
    Ok(())
}

fn object_features(path: &Path, format: &str, blockers: &mut Vec<String>) -> Result<()> {
    let object_ids: Vec<String> = git_text(
        Some(path),
        &["rev-list", "--objects", "--all", "--no-object-names"],
    )?
    .lines()
    .map(str::to_owned)
    .collect();
    if object_ids.len() > MAX_REACHABLE_OBJECTS {
        return Err(CoreError::blocked(format!("This preview can inspect at most {MAX_REACHABLE_OBJECTS} reachable objects. No repository data was changed.")));
    }
    if object_ids.is_empty() {
        return Ok(());
    }
    let summary = git(
        Some(path),
        [
            "cat-file",
            "--batch-check=%(objectname) %(objecttype) %(objectsize)",
        ],
        Some(format!("{}\n", object_ids.join("\n")).into_bytes()),
    )?;
    let mut trees = Vec::new();
    let mut small_blobs = Vec::new();
    for line in String::from_utf8_lossy(&summary).lines() {
        let fields: Vec<_> = line.split(' ').collect();
        if fields.len() != 3 {
            return Err(CoreError::blocked("Missing objects in history"));
        }
        if fields[1] == "tree" {
            trees.push(fields[0].to_string());
        }
        if fields[1] == "blob" && fields[2].parse::<u64>().unwrap_or(u64::MAX) <= 1024 {
            small_blobs.push(fields[0].to_string());
        }
    }
    let oid_len = if format == "sha256" { 32 } else { 20 };
    let mut has_submodules = false;
    let mut has_workflows = false;
    let mut github_trees = BTreeSet::new();
    let mut tree_entries = BTreeMap::new();
    for (oid, bytes) in trees.iter().zip(batch(path, &trees)?) {
        let mut cursor = 0;
        let mut entries = Vec::new();
        while cursor < bytes.len() {
            let nul = bytes[cursor..]
                .iter()
                .position(|b| *b == 0)
                .ok_or_else(|| CoreError::blocked("Malformed tree"))?
                + cursor;
            if nul + 1 + oid_len > bytes.len() {
                return Err(CoreError::blocked("Truncated tree"));
            }
            let item = &bytes[cursor..nul];
            let space = item
                .iter()
                .position(|b| *b == b' ')
                .ok_or_else(|| CoreError::blocked("Malformed tree entry"))?;
            let mode = &item[..space];
            let name = item[space + 1..].to_vec();
            let target: String = bytes[nul + 1..nul + 1 + oid_len]
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect();
            has_submodules |= mode == b"160000";
            if name == b".github" && mode == b"40000" {
                github_trees.insert(target.clone());
            }
            entries.push((name, mode.to_vec(), target));
            cursor = nul + 1 + oid_len;
        }
        tree_entries.insert(oid.clone(), entries);
    }
    for oid in github_trees {
        if tree_entries.get(&oid).is_some_and(|entries| {
            entries
                .iter()
                .any(|(name, mode, _)| name == b"workflows" && mode == b"40000")
        }) {
            has_workflows = true;
        }
    }
    if has_submodules {
        blockers.push("Submodules in exported history are inspection-only".into());
    }
    if has_workflows {
        blockers.push("Workflow history awaits the permissions feasibility gate".into());
    }
    if batch(path, &small_blobs)?
        .iter()
        .any(|blob| blob.starts_with(b"version https://git-lfs.github.com/spec/v1\n"))
    {
        blockers.push("LFS pointers in exported history are inspection-only".into());
    }
    Ok(())
}

pub fn scan(path: &Path) -> Result<Scan> {
    let canonical = path.canonicalize()?;
    let path = canonical.as_path();
    let object_format = git_text(Some(path), &["rev-parse", "--show-object-format"])?;
    let baseline = refs(path)?;
    let default_ref = git_text(Some(path), &["symbolic-ref", "--quiet", "HEAD"]).ok();
    let mut blockers = Vec::new();
    inspect_storage(path, &object_format, &mut blockers)?;
    let branches: Vec<_> = baseline
        .keys()
        .filter(|r| r.starts_with("refs/heads/"))
        .collect();
    if branches.len() != 1 {
        blockers.push("Writer requires exactly one branch".into());
    }
    if default_ref
        .as_ref()
        .is_none_or(|r| !baseline.contains_key(r) || !r.starts_with("refs/heads/"))
    {
        blockers.push("An existing default branch is required".into());
    }
    if baseline.keys().any(|r| !r.starts_with("refs/heads/")) {
        blockers.push("Tags and additional refs are inspection-only".into());
    }
    let mut commits = Vec::new();
    for (oid, raw) in raw_commits(path)? {
        let commit = parse_commit(oid, raw)?;
        if !commit.unsupported_headers.is_empty() {
            blockers.push(format!(
                "Unsupported headers or signatures at {}",
                commit.oid
            ));
        }
        commits.push(commit);
    }
    if commits.is_empty() {
        blockers.push("Empty repositories cannot be rewritten".into());
    }
    if commits.len() > MAX_PREPARE_COMMITS {
        blockers.push(format!("This preview limits local preparation to {MAX_PREPARE_COMMITS} reachable commits; this history remains inspection-only"));
    }
    object_features(path, &object_format, &mut blockers)?;
    if refs(path)? != baseline {
        return Err(CoreError::blocked(
            "Repository refs moved during scan; scan again",
        ));
    }
    blockers.sort();
    blockers.dedup();
    let mut result = Scan {
        id: String::new(),
        source_path: path.to_string_lossy().into(),
        object_format,
        default_ref,
        refs: baseline,
        commits,
        blockers,
        toolchain: Toolchain {
            git_version: git_text(None, &["--version"])?,
            worker_version: FILTER_REPO_VERSION.into(),
            worker_sha256: FILTER_REPO_SHA256.into(),
            python_version: python_version().unwrap_or_default(),
            callback_sha256: digest(include_bytes!("../../../../workers/rewrite/worker.py")),
        },
    };
    result.id = digest(&serde_json::to_vec(&result)?);
    Ok(result)
}

fn validate_email(email: &str) -> Result<()> {
    if email.len() > 254
        || !email.contains('@')
        || email
            .bytes()
            .any(|b| b <= b' ' || b >= 127 || b == b'<' || b == b'>')
    {
        return Err(CoreError::invalid(
            "Email must be an explicit ASCII address without whitespace or delimiters",
        ));
    }
    Ok(())
}

pub(crate) fn hash_plan(plan: &Plan) -> Result<String> {
    let mut value = plan.clone();
    value.id.clear();
    value.content_hash.clear();
    Ok(digest(&serde_json::to_vec(&value)?))
}

pub fn plan(scan: &Scan, selection: &Selection) -> Result<Plan> {
    validate_email(&selection.confirmed_email)?;
    validate_email(&selection.replacement_email)?;
    if selection.confirmed_email == selection.replacement_email {
        return Err(CoreError::invalid(
            "Replacement must change the confirmed author email",
        ));
    }
    if !selection.replacement_email.ends_with(".invalid") {
        return Err(CoreError::invalid(
            "Prototype replacement email must use a reserved .invalid domain",
        ));
    }
    let selected: BTreeSet<String> = selection.commit_oids.iter().cloned().collect();
    if selected.is_empty() {
        return Err(CoreError::invalid("Select at least one exact commit OID"));
    }
    if selected.len() != selection.commit_oids.len() {
        return Err(CoreError::invalid("Duplicate selected commit OIDs"));
    }
    let lookup: BTreeMap<_, _> = scan.commits.iter().map(|c| (c.oid.clone(), c)).collect();
    for oid in &selected {
        let commit = lookup.get(oid).ok_or_else(|| {
            CoreError::invalid("Selection includes a commit absent from this scan")
        })?;
        let raw = STANDARD
            .decode(&commit.raw_base64)
            .map_err(|_| CoreError::invalid("Invalid raw commit payload"))?;
        let (headers, _) = split_raw(&raw)?;
        let author = headers
            .split(|b| *b == b'\n')
            .find_map(|line| line.strip_prefix(b"author "))
            .ok_or_else(|| CoreError::invalid("Missing author"))?;
        if identity(author)?.1 != selection.confirmed_email.as_bytes() {
            return Err(CoreError::blocked(
                "Selected author email does not match the explicitly confirmed identity",
            ));
        }
    }
    let mut children: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for commit in &scan.commits {
        for parent in &commit.parents {
            children.entry(parent).or_default().push(&commit.oid);
        }
    }
    let mut affected = selected.clone();
    let mut queue: Vec<String> = selected.iter().cloned().collect();
    while let Some(oid) = queue.pop() {
        for child in children.get(oid.as_str()).into_iter().flatten() {
            if affected.insert((*child).into()) {
                queue.push((*child).into());
            }
        }
    }
    let mut selection = selection.clone();
    selection.commit_oids = selected.iter().cloned().collect();
    let mut result = Plan {
        id: String::new(),
        content_hash: String::new(),
        version: SCHEMA_VERSION,
        scan: scan.clone(),
        selection,
        selected_oids: selected.into_iter().collect(),
        affected_oids: affected.into_iter().collect(),
        blockers: scan.blockers.clone(),
    };
    result.content_hash = hash_plan(&result)?;
    result.id = result.content_hash.clone();
    Ok(result)
}
