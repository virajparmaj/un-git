use crate::{CoreError, Result};
use fs2::FileExt;
use rusqlite::{params, Connection, OpenFlags, OptionalExtension};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEvent {
    pub sequence: i64,
    pub operation_id: String,
    pub state: String,
    pub at: String,
}

pub struct RepositoryLock {
    file: File,
}
impl Drop for RepositoryLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn private_directory(path: &Path) -> Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            return Err(CoreError::blocked(
                "Private data directories cannot be symbolic links or files",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    fs::create_dir_all(path)?;
    if !fs::symlink_metadata(path)?.is_dir() {
        return Err(CoreError::blocked(
            "Private data directory changed during creation",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(crate) fn open_private_lock(path: &Path) -> Result<File> {
    private_directory(
        path.parent()
            .ok_or_else(|| CoreError::invalid("Lock has no parent"))?,
    )?;
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(CoreError::blocked(
            "Lock must be an independent regular file",
        ));
    }
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    Ok(file)
}

pub fn durable_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CoreError::invalid("Artifact has no parent"))?;
    private_directory(parent)?;
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(&mut file, value)?;
    file.write_all(b"\n")?;
    file.as_file().sync_all()?;
    file.persist(path)
        .map_err(|e| CoreError::local(e.to_string()))?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

pub fn default_data_dir() -> PathBuf {
    if let Some(path) = std::env::var_os("UN_GIT_DATA_DIR") {
        return PathBuf::from(path);
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/com.virajparmaj.un-git")
    }
    #[cfg(not(target_os = "macos"))]
    {
        home.join(".local/share/un-git")
    }
}

impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        private_directory(path)?;
        let store = Self {
            root: fs::canonicalize(path)?,
        };
        private_directory(&store.root.join("operations"))?;
        private_directory(&store.root.join("locks"))?;
        let mut connection = store.connection()?;
        let version: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if version > 1 {
            return Err(CoreError::blocked(
                "This database requires a newer un-git version; data was preserved.",
            ));
        }
        if version == 0 {
            let tx = connection.transaction()?;
            tx.execute_batch("CREATE TABLE records (kind TEXT NOT NULL, id TEXT NOT NULL, body TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY(kind,id)); CREATE TABLE events (sequence INTEGER PRIMARY KEY AUTOINCREMENT, operation_id TEXT NOT NULL, state TEXT NOT NULL, at TEXT NOT NULL); CREATE INDEX events_operation ON events(operation_id, sequence); PRAGMA user_version=1;")?;
            tx.commit()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(
                store.root.join("un-git.sqlite3"),
                fs::Permissions::from_mode(0o600),
            )?;
        }
        Ok(store)
    }

    fn connection(&self) -> Result<Connection> {
        let path = self.root.join("un-git.sqlite3");
        for extension in ["", "-wal", "-shm", "-journal"] {
            let path = self.root.join(format!("un-git.sqlite3{extension}"));
            match fs::symlink_metadata(path) {
                Ok(metadata) if !metadata.is_file() || metadata.nlink() != 1 => {
                    return Err(CoreError::blocked(
                        "Database files must be independent regular files",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let connection = Connection::open_with_flags(
            path,
            OpenFlags::default() | OpenFlags::SQLITE_OPEN_NOFOLLOW,
        )?;
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.execute_batch(
            "PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA foreign_keys=ON;",
        )?;
        Ok(connection)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn put(&self, kind: &str, id: &str, value: &impl Serialize) -> Result<()> {
        self.connection()?.execute("INSERT INTO records(kind,id,body,updated_at) VALUES (?1,?2,?3,?4) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated_at=excluded.updated_at", params![kind,id,serde_json::to_string(value)?,now()])?;
        Ok(())
    }

    pub fn get<T: DeserializeOwned>(&self, kind: &str, id: &str) -> Result<T> {
        let body: Option<String> = self
            .connection()?
            .query_row(
                "SELECT body FROM records WHERE kind=?1 AND id=?2",
                params![kind, id],
                |row| row.get(0),
            )
            .optional()?;
        serde_json::from_str(
            &body.ok_or_else(|| CoreError::invalid(format!("Unknown {kind} identifier")))?,
        )
        .map_err(Into::into)
    }

    pub fn list<T: DeserializeOwned>(&self, kind: &str) -> Result<Vec<T>> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT body FROM records WHERE kind=?1 ORDER BY updated_at DESC")?;
        let rows = statement.query_map([kind], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    pub fn checkpoint(&self, id: &str, state: &str, value: &impl Serialize) -> Result<()> {
        let mut connection = self.connection()?;
        let tx = connection.transaction()?;
        tx.execute("INSERT INTO records(kind,id,body,updated_at) VALUES ('operation',?1,?2,?3) ON CONFLICT(kind,id) DO UPDATE SET body=excluded.body,updated_at=excluded.updated_at", params![id,serde_json::to_string(value)?,now()])?;
        tx.execute(
            "INSERT INTO events(operation_id,state,at) VALUES (?1,?2,?3)",
            params![id, state, now()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn events(&self, operation_id: &str) -> Result<Vec<JournalEvent>> {
        let connection = self.connection()?;
        let mut statement = connection.prepare("SELECT sequence,operation_id,state,at FROM events WHERE operation_id=?1 ORDER BY sequence")?;
        let rows = statement.query_map([operation_id], |row| {
            Ok(JournalEvent {
                sequence: row.get(0)?,
                operation_id: row.get(1)?,
                state: row.get(2)?,
                at: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn repository_lock(&self, repository: &str) -> Result<RepositoryLock> {
        let key = format!("{:x}", Sha256::digest(repository.as_bytes()));
        let path = self.root.join("locks").join(format!("{key}.lock"));
        let file = open_private_lock(&path)?;
        file.try_lock_exclusive().map_err(|_| {
            CoreError::blocked("Another desktop or CLI operation is using this repository.")
        })?;
        Ok(RepositoryLock { file })
    }

    pub fn operation_dir(&self, id: &str) -> Result<PathBuf> {
        uuid::Uuid::parse_str(id)
            .map_err(|_| CoreError::invalid("Invalid operation identifier"))?;
        let parent = self.root.join("operations");
        private_directory(&parent)?;
        let path = parent.join(id);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(CoreError::blocked(
                    "Operation directory must remain a real directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_events_and_locks_survive_reopen() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        store
            .checkpoint(
                "operation",
                "PLANNED",
                &serde_json::json!({"state":"PLANNED"}),
            )
            .unwrap();
        let lock = store.repository_lock("/repo").unwrap();
        assert!(Store::open(temp.path())
            .unwrap()
            .repository_lock("/repo")
            .is_err());
        drop(lock);
        assert!(store.repository_lock("/repo").is_ok());
        assert_eq!(store.events("operation").unwrap().len(), 1);
        assert_eq!(
            store
                .get::<serde_json::Value>("operation", "operation")
                .unwrap()["state"],
            "PLANNED"
        );
        assert!(store.operation_dir("../escape").is_err());
    }
    #[test]
    fn future_database_is_preserved_and_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(temp.path()).unwrap();
        store
            .connection()
            .unwrap()
            .execute_batch("PRAGMA user_version=99;")
            .unwrap();
        assert!(Store::open(temp.path()).is_err());
    }

    #[test]
    fn private_data_rejects_redirected_database_lock_and_operation_paths() {
        let temp = tempfile::tempdir().unwrap();
        let store = Store::open(&temp.path().join("app")).unwrap();
        let external = temp.path().join("external");
        fs::create_dir(&external).unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        std::os::unix::fs::symlink(&external, store.root.join("operations").join(&id)).unwrap();
        assert!(store.operation_dir(&id).is_err());
        let linked = temp.path().join("linked");
        std::os::unix::fs::symlink(&external, &linked).unwrap();
        assert!(private_directory(&linked).is_err());
        let external_file = external.join("data");
        fs::write(&external_file, "untouched").unwrap();
        let lock = store.root.join("locks/redirect.lock");
        std::os::unix::fs::symlink(&external_file, &lock).unwrap();
        assert!(open_private_lock(&lock).is_err());
        fs::remove_file(store.root.join("un-git.sqlite3")).unwrap();
        std::os::unix::fs::symlink(&external_file, store.root.join("un-git.sqlite3")).unwrap();
        assert!(store.connection().is_err());
        assert_eq!(fs::read_to_string(external_file).unwrap(), "untouched");
    }
}
