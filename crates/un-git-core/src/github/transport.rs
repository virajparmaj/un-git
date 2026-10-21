//! Fixed Inspector-only HTTPS reads into immutable, app-owned bare snapshots.
use super::{helper::CredentialBroker, GithubClient, GithubRepository};
use crate::{
    engine::{
        self,
        runner::{self, NetworkLimits},
    },
    store::{durable_json, private_directory, Store},
    CoreError, Result,
};
use chrono::Utc;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File},
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    process::Command,
};
use uuid::Uuid;

const IMPORT_VERSION: u32 = 1;
const MAX_IMPORT_BYTES: u64 = 1024 * 1024 * 1024;
const DISK_RESERVE: u64 = 256 * 1024 * 1024;
const RECORD_KIND: &str = "github_import_v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImportedRepository {
    pub id: String,
    pub repository_id: u64,
    pub repository_full_name: String,
    pub repository_path: String,
    pub fetched_at: i64,
}

#[derive(Serialize)]
struct ImportRecord {
    version: u32,
    state: String,
    account_id: u64,
    client_id: String,
    repository: ImportedRepository,
    scope: &'static str,
    baseline: Option<RemoteRefs>,
    error_code: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
struct RemoteRefs {
    default_ref: Option<String>,
    refs: BTreeMap<String, String>,
}

fn import_error(message: &str) -> CoreError {
    CoreError::new("IMPORT_BLOCKED", message)
}

fn canonical_remote(repository: &GithubRepository) -> Result<(String, String)> {
    let Some((owner, name)) = repository.full_name.split_once('/') else {
        return Err(import_error("GitHub returned an invalid repository name."));
    };
    let valid = |part: &str| {
        !part.is_empty()
            && part.len() <= 100
            && !matches!(part, "." | "..")
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    };
    if !valid(owner) || !valid(name) || owner != repository.owner.login || name != repository.name {
        return Err(import_error(
            "GitHub returned an invalid repository identity.",
        ));
    }
    let path = format!("{owner}/{name}.git");
    let url = format!("https://github.com/{path}");
    if repository.clone_url != url {
        return Err(import_error(
            "Repository address changed or is unsupported. Refresh repositories before importing.",
        ));
    }
    Ok((url, path))
}

fn parse_refs(output: &[u8]) -> Result<RemoteRefs> {
    let text = std::str::from_utf8(output)
        .map_err(|_| import_error("Unsupported remote ref response."))?;
    let mut refs = BTreeMap::new();
    let mut default_ref = None;
    let mut head_oid = None;
    for (index, line) in text.lines().enumerate() {
        if index >= 20_000 {
            return Err(import_error(
                "Repository ref inventory exceeds the preview limit.",
            ));
        }
        let (value, name) = line
            .split_once('\t')
            .ok_or_else(|| import_error("Malformed remote ref response."))?;
        if let Some(target) = value.strip_prefix("ref: ") {
            if name != "HEAD"
                || !target.starts_with("refs/heads/")
                || default_ref.replace(target.to_owned()).is_some()
            {
                return Err(import_error("Unsupported default-branch advertisement."));
            }
            continue;
        }
        if !runner::checked_oid(value) || value.len() != 40 {
            return Err(import_error(
                "Inspector imports currently require SHA-1 Git objects.",
            ));
        }
        if name == "HEAD" {
            if head_oid.replace(value.to_owned()).is_some() {
                return Err(import_error("Duplicate remote HEAD."));
            }
        } else if name.starts_with("refs/tags/") && name.ends_with("^{}") {
            // A peeled annotated tag is not an additional ref.
        } else if name.starts_with("refs/")
            && !name.bytes().any(|b| b.is_ascii_control() || b == b' ')
        {
            if refs.insert(name.to_owned(), value.to_owned()).is_some() {
                return Err(import_error("Duplicate remote ref."));
            }
        } else {
            return Err(import_error("Unsupported remote ref advertisement."));
        }
    }
    if !refs.is_empty() && default_ref.as_ref().and_then(|r| refs.get(r)) != head_oid.as_ref() {
        return Err(import_error(
            "Remote default branch is unresolved; refresh and retry.",
        ));
    }
    if !refs.is_empty() && (default_ref.is_none() || head_oid.is_none()) {
        return Err(import_error("Remote default branch is missing."));
    }
    Ok(RemoteRefs { default_ref, refs })
}

fn checked_git_exec_path(executable: &Path) -> Result<PathBuf> {
    let path = runner::git_text(None, &["--exec-path"])?;
    let path = PathBuf::from(path);
    if !path.is_absolute() || !path.join("git-remote-https").is_file() {
        return Err(import_error("The checked Git HTTPS helper is unavailable."));
    }
    // The checked executable itself chooses its helpers with all inherited Git
    // variables removed. No repository or PATH-derived helper directory is used.
    let _ = executable;
    Ok(path.canonicalize()?)
}

enum ReadCommand {
    Refs,
    Fetch,
}

fn read_remote(
    command_kind: ReadCommand,
    repository: &Path,
    url: &str,
    credential_path: &str,
    token: &str,
    limits: &NetworkLimits,
) -> Result<Vec<u8>> {
    let executable = runner::git_executable()?;
    let exec_path = checked_git_exec_path(&executable)?;
    let mut command = Command::new(executable);
    runner::isolated(&mut command);
    command
        .env("GIT_ALLOW_PROTOCOL", "https")
        .env("GIT_EXEC_PATH", exec_path);
    let broker = CredentialBroker::new(credential_path.into(), token.into())?;
    broker.configure(&mut command, &std::env::current_exe()?.canonicalize()?)?;
    for configuration in [
        "credential.useHttpPath=true",
        "credential.interactive=false",
        "http.sslVerify=true",
        "http.followRedirects=false",
        "http.proxy=",
        "http.extraHeader=",
        "http.cookieFile=",
        "http.saveCookies=false",
        "http.lowSpeedLimit=1024",
        "http.lowSpeedTime=30",
        "http.maxRequests=1",
        "fetch.recurseSubmodules=false",
        "submodule.recurse=false",
        "gc.auto=0",
        "maintenance.auto=false",
        "fetch.writeCommitGraph=false",
        "transfer.fsckObjects=true",
        "fetch.fsckObjects=true",
        "transfer.bundleURI=false",
        "promisor.acceptFromServer=none",
        "fetch.unpackLimit=1",
        "protocol.version=2",
    ] {
        command.arg("-c").arg(configuration);
    }
    command.arg("-C").arg(repository);
    match command_kind {
        ReadCommand::Refs => {
            command.args(["ls-remote", "--symref", "--", url]);
        }
        ReadCommand::Fetch => {
            command.args([
                "fetch",
                "--no-recurse-submodules",
                "--no-auto-maintenance",
                "--no-write-fetch-head",
                "--no-tags",
                "--",
                url,
                "+refs/*:refs/*",
            ]);
        }
    }
    // The broker is dropped after Git and all pipe-owning descendants finish.
    runner::execute_network(command, limits)
}

fn persist_record(store: &Store, directory: &Path, record: &ImportRecord) -> Result<()> {
    durable_json(&directory.join("import.json"), record)?;
    // Every newly created ancestor must be durable before the database points
    // at the artifact. Syncing only import.json's parent does not persist the
    // UUID directory entry in imports or imports' entry in Application Support.
    File::open(
        directory
            .parent()
            .ok_or_else(|| import_error("Missing import parent."))?,
    )?
    .sync_all()?;
    File::open(store.root())?.sync_all()?;
    store.put(RECORD_KIND, &record.repository.id, record)
}

fn repository_import_lock(credential_lock_dir: &Path, repository_id: u64) -> Result<File> {
    let parent = credential_lock_dir
        .parent()
        .ok_or_else(|| import_error("Missing global lock directory."))?;
    private_directory(parent)?;
    let directory = parent.join("repository-import-locks");
    private_directory(&directory)?;
    let lock =
        crate::store::open_private_lock(&directory.join(format!("github-{repository_id}.lock")))?;
    lock.try_lock_exclusive().map_err(|_| {
        CoreError::new(
            "IMPORT_BUSY",
            "Another desktop or CLI process is importing this repository.",
        )
    })?;
    Ok(lock)
}

fn sync_private_tree(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink()
        || (!metadata.is_dir() && !metadata.is_file())
        || (metadata.is_file() && metadata.nlink() != 1)
    {
        return Err(import_error(
            "Import artifacts must use independent regular storage.",
        ));
    }
    if metadata.is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
        for child in fs::read_dir(path)? {
            sync_private_tree(&child?.path())?;
        }
    } else {
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    }
    File::open(path)?.sync_all()?;
    Ok(())
}

fn finish_import(
    store: &Store,
    directory: &Path,
    mut record: ImportRecord,
) -> Result<ImportedRepository> {
    // File and directory durability precede both READY and the lookup exposed to callers.
    sync_private_tree(directory)?;
    record.repository.fetched_at = Utc::now().timestamp();
    record.state = "READY".into();
    persist_record(store, directory, &record)?;
    store.put(
        "imported_repository",
        &record.repository.repository_path,
        &record.repository,
    )?;
    Ok(record.repository)
}

fn download_snapshot(
    store: &Store,
    directory: &Path,
    record: &mut ImportRecord,
    metadata: &GithubRepository,
    token: &str,
) -> Result<()> {
    let repository = Path::new(&record.repository.repository_path);
    private_directory(repository)?;
    let limits = NetworkLimits {
        directory: directory.into(),
        maximum_bytes: MAX_IMPORT_BYTES,
        minimum_free_bytes: DISK_RESERVE,
    };
    if fs2::available_space(directory)? < MAX_IMPORT_BYTES + DISK_RESERVE {
        return Err(import_error(
            "Import requires at least 1.25 GiB of free space in app storage.",
        ));
    }
    runner::git(
        Some(repository),
        ["init", "--bare", "--template=", "--initial-branch=main"],
        None,
    )?;
    let (url, credential_path) = canonical_remote(metadata)?;
    let baseline = parse_refs(&read_remote(
        ReadCommand::Refs,
        repository,
        &url,
        &credential_path,
        token,
        &limits,
    )?)?;
    if baseline.default_ref.as_deref()
        != metadata
            .default_branch
            .as_ref()
            .map(|b| format!("refs/heads/{b}"))
            .as_deref()
        && !baseline.refs.is_empty()
    {
        return Err(import_error(
            "GitHub default branch changed during import. Refresh repositories and retry.",
        ));
    }
    record.baseline = Some(baseline.clone());
    persist_record(store, directory, record)?;
    read_remote(
        ReadCommand::Fetch,
        repository,
        &url,
        &credential_path,
        token,
        &limits,
    )?;
    limits.check()?;
    if let Some(default_ref) = &baseline.default_ref {
        runner::git(
            Some(repository),
            ["symbolic-ref", "HEAD", default_ref],
            None,
        )?;
    }
    let observed = parse_refs(&read_remote(
        ReadCommand::Refs,
        repository,
        &url,
        &credential_path,
        token,
        &limits,
    )?)?;
    if observed != baseline {
        return Err(import_error("Remote refs changed during import. The incomplete snapshot was retained; retry for a fresh snapshot."));
    }
    runner::git(
        Some(repository),
        ["fsck", "--full", "--strict", "--no-reflogs"],
        None,
    )?;
    let scan = engine::scan(repository)?;
    if scan.refs != baseline.refs
        || (!baseline.refs.is_empty() && scan.default_ref != baseline.default_ref)
    {
        return Err(import_error(
            "Fetched objects and advertised refs do not match.",
        ));
    }
    limits.check()?;
    Ok(())
}

impl GithubClient {
    /// Read installed repository objects into a new immutable local snapshot.
    /// This accepts a stable API identity, never an arbitrary remote or path.
    pub async fn import_repository(&self, repository_id: u64) -> Result<ImportedRepository> {
        let token = self.access_token().await?;
        let account = self.account_for_token(&token).await?;
        let inventory = self.repositories_for_token(&token).await?;
        let metadata = inventory
            .repositories
            .into_iter()
            .find(|r| r.id == repository_id && r.contents_read)
            .ok_or_else(|| {
                import_error("Select an installed repository with verified Contents read access.")
            })?;
        canonical_remote(&metadata)?;
        // This lock lives beside credential locks, outside alternate data dirs.
        let lock = repository_import_lock(&self.credential_lock_dir, repository_id)?;
        let store = Store::open(&self.data_dir)?;
        let imports = store.root().join("imports");
        private_directory(&imports)?;
        let id = Uuid::new_v4().to_string();
        let directory = imports.join(&id);
        private_directory(&directory)?;
        let record = ImportRecord {
            version: IMPORT_VERSION,
            state: "FETCHING".into(),
            account_id: account.id,
            client_id: self.client_id.clone(),
            repository: ImportedRepository {
                id,
                repository_id,
                repository_full_name: metadata.full_name.clone(),
                repository_path: directory.join("repository.git").to_string_lossy().into(),
                fetched_at: 0,
            },
            scope: "All refs advertised by GitHub during this import; hidden refs and external clones are not inventoried.",
            baseline: None,
            error_code: None,
        };
        persist_record(&store, &directory, &record)?;
        let metadata_before = metadata.clone();
        let (store, directory, mut record, lock) = tokio::task::spawn_blocking(move || {
            let mut record = record;
            if let Err(error) =
                download_snapshot(&store, &directory, &mut record, &metadata, &token)
            {
                record.state = "FAILED".into();
                record.error_code = Some(error.code.clone());
                persist_record(&store, &directory, &record)?;
                return Err(error);
            }
            Ok((store, directory, record, lock))
        })
        .await
        .map_err(|_| {
            import_error("Import worker ended unexpectedly. Incomplete files were retained.")
        })??;

        // A switch/logout during a download cannot silently attach old-account
        // repository data to the new account's ready inventory.
        let authorization = async {
            let token = self.access_token().await?;
            let current = self.account_for_token(&token).await?;
            if current.id != record.account_id {
                return Err(import_error("GitHub account changed during import."));
            }
            let repos = self.repositories_for_token(&token).await?;
            let current = repos
                .repositories
                .iter()
                .find(|r| r.id == repository_id && r.contents_read)
                .ok_or_else(|| import_error("Repository access changed during import."))?;
            if current.full_name != metadata_before.full_name
                || current.clone_url != metadata_before.clone_url
                || current.default_branch != metadata_before.default_branch
            {
                return Err(import_error(
                    "Repository identity or default branch changed during import.",
                ));
            }
            Ok(())
        }
        .await;
        if let Err(error) = authorization {
            record.state = "FAILED".into();
            record.error_code = Some(error.code.clone());
            persist_record(&store, &directory, &record)?;
            return Err(error);
        }
        tokio::task::spawn_blocking(move || {
            // Keep ownership if the awaiting UI task is cancelled while this
            // blocking finalization continues.
            let _lock = lock;
            finish_import(&store, &directory, record)
        })
        .await
        .map_err(|_| import_error("Import finalization ended unexpectedly."))?
    }
}

#[cfg(test)]
mod tests {
    use super::super::GithubAccount;
    use super::*;

    #[test]
    fn repository_locks_coordinate_alternate_data_directories_and_reject_symlinks() {
        let temp = tempfile::tempdir().unwrap();
        let credentials = temp.path().join("shared/credential-locks");
        let first_store = Store::open(&temp.path().join("desktop")).unwrap();
        let second_store = Store::open(&temp.path().join("cli-alternate")).unwrap();
        assert_ne!(first_store.root(), second_store.root());
        let lock = repository_import_lock(&credentials, 123).unwrap();
        assert_eq!(
            repository_import_lock(&credentials, 123).unwrap_err().code,
            "IMPORT_BUSY"
        );
        assert!(repository_import_lock(&credentials, 456).is_ok());
        drop(lock);
        assert!(repository_import_lock(&credentials, 123).is_ok());
        std::os::unix::fs::symlink(
            temp.path(),
            temp.path()
                .join("shared/repository-import-locks/github-789.lock"),
        )
        .unwrap();
        assert!(repository_import_lock(&credentials, 789).is_err());
    }

    #[test]
    fn unsafe_artifacts_cannot_become_ready_or_enter_the_import_lookup() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        let id = Uuid::new_v4().to_string();
        let directory = store.root().join("imports").join(&id);
        private_directory(&directory).unwrap();
        let path = directory
            .join("repository.git")
            .to_string_lossy()
            .into_owned();
        let record = ImportRecord {
            version: IMPORT_VERSION,
            state: "FETCHING".into(),
            account_id: 1,
            client_id: "fixture".into(),
            scope: "Generated fixture",
            repository: ImportedRepository {
                id: id.clone(),
                repository_id: 1,
                repository_full_name: "fixture/repo".into(),
                repository_path: path.clone(),
                fetched_at: 0,
            },
            baseline: None,
            error_code: None,
        };
        persist_record(&store, &directory, &record).unwrap();
        std::os::unix::fs::symlink(temp.path(), directory.join("redirect")).unwrap();
        assert!(finish_import(&store, &directory, record).is_err());
        let saved: serde_json::Value = store.get(RECORD_KIND, &id).unwrap();
        assert_eq!(saved["state"], "FETCHING");
        assert!(store
            .get::<ImportedRepository>("imported_repository", &path)
            .is_err());
    }

    fn repository() -> GithubRepository {
        GithubRepository {
            id: 1,
            installation_id: 2,
            name: "fixture".into(),
            full_name: "owner/fixture".into(),
            owner: GithubAccount {
                id: 1,
                login: "owner".into(),
            },
            private: true,
            fork: false,
            archived: false,
            default_branch: Some("main".into()),
            clone_url: "https://github.com/owner/fixture.git".into(),
            contents_read: true,
            contents_write: false,
            administration_read: false,
            pull_requests_read: true,
            personal_owner: true,
            permission_status: "intersection_observed".into(),
        }
    }

    #[test]
    fn remote_identity_never_accepts_a_caller_supplied_host_or_path() {
        assert_eq!(
            canonical_remote(&repository()).unwrap().0,
            "https://github.com/owner/fixture.git"
        );
        for url in [
            "https://github.com.evil/owner/fixture.git",
            "https://token@github.com/owner/fixture.git",
            "https://github.com:443/owner/fixture.git",
            "https://github.com/owner/fixture.git?token=x",
            "https://github.com/owner/../other.git",
            "https://github.com/owner/fixture.git#x",
            "file:///tmp/repo",
            "https://github.com/owner/%66ixture.git",
        ] {
            let mut metadata = repository();
            metadata.clone_url = url.into();
            assert!(canonical_remote(&metadata).is_err());
        }
        for name in [
            "../other",
            "owner/repo/extra",
            "owner/..",
            "owner/with\nnewline",
            "other/fixture",
        ] {
            let mut metadata = repository();
            metadata.full_name = name.into();
            assert!(canonical_remote(&metadata).is_err());
        }
    }

    #[test]
    fn refs_preserve_advertised_scope_and_reject_unresolved_defaults() {
        let oid = "1".repeat(40);
        let listing = format!("ref: refs/heads/main\tHEAD\n{oid}\tHEAD\n{oid}\trefs/heads/main\n{oid}\trefs/tags/v1\n{oid}\trefs/tags/v1^{{}}\n{oid}\trefs/pull/1/head\n");
        let refs = parse_refs(listing.as_bytes()).unwrap();
        assert_eq!(refs.refs.len(), 3);
        assert!(refs.refs.contains_key("refs/pull/1/head"));
        assert!(parse_refs(format!("{oid}\trefs/heads/main\n").as_bytes()).is_err());
        assert!(parse_refs(
            format!("ref: refs/heads/other\tHEAD\n{oid}\tHEAD\n{oid}\trefs/heads/main\n")
                .as_bytes()
        )
        .is_err());
        assert!(parse_refs(&[]).unwrap().refs.is_empty());
    }

    #[test]
    fn import_storage_rejects_oversize_and_symlink_files() {
        let temp = tempfile::tempdir().unwrap();
        let limits = NetworkLimits {
            directory: temp.path().into(),
            maximum_bytes: 1024,
            minimum_free_bytes: 0,
        };
        fs::write(temp.path().join("pack"), vec![0; 1025]).unwrap();
        assert!(limits.check().is_err());
        fs::remove_file(temp.path().join("pack")).unwrap();
        std::os::unix::fs::symlink("/tmp", temp.path().join("external")).unwrap();
        assert!(limits.check().is_err());
    }

    #[test]
    fn network_failures_do_not_return_untrusted_stderr() {
        let temp = tempfile::tempdir().unwrap();
        let limits = NetworkLimits {
            directory: temp.path().into(),
            maximum_bytes: 1024,
            minimum_free_bytes: 0,
        };
        let mut command = Command::new("/bin/sh");
        runner::isolated(&mut command);
        command.args(["-c", "printf 'private-fixture-error' >&2; exit 1"]);
        let error = runner::execute_network(command, &limits).unwrap_err();
        assert_eq!(error.code, "GIT_READ_FAILED");
        assert!(!error.message.contains("private-fixture-error"));
    }
}
