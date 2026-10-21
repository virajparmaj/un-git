//! Conservative developer-preview limits, enforced before expensive local work.
use super::runner::git_text;
use crate::{CoreError, Result};
use std::{fs, path::Path};

pub const MAX_SCAN_COMMITS: usize = 100_000;
pub const MAX_PREPARE_COMMITS: usize = 10_000;
pub const MAX_REACHABLE_OBJECTS: usize = 500_000;
pub const MAX_SOURCE_OBJECT_BYTES: u64 = 1024 * 1024 * 1024;
const DISK_RESERVE: u64 = 256 * 1024 * 1024;

fn object_storage_bytes(path: &Path) -> Result<u64> {
    let mut pending = vec![path.to_path_buf()];
    let mut bytes = 0u64;
    let mut entries = 0usize;
    while let Some(path) = pending.pop() {
        let metadata = fs::symlink_metadata(&path)?;
        entries += 1;
        if entries > MAX_REACHABLE_OBJECTS * 2 {
            return Err(CoreError::blocked(
                "Object storage exceeds the preview file limit",
            ));
        }
        if metadata.file_type().is_symlink() || (!metadata.is_dir() && !metadata.is_file()) {
            return Err(CoreError::blocked(
                "Preparation requires regular, independent object storage",
            ));
        }
        if metadata.is_dir() {
            for entry in fs::read_dir(path)? {
                pending.push(entry?.path());
            }
        } else {
            bytes = bytes
                .checked_add(metadata.len())
                .ok_or_else(|| CoreError::blocked("Object storage size overflow"))?;
            if bytes > MAX_SOURCE_OBJECT_BYTES {
                return Err(CoreError::blocked("Local preparation supports at most 1 GiB of source object storage in this preview"));
            }
        }
    }
    Ok(bytes)
}

fn check_space(source_bytes: u64, available_bytes: u64) -> Result<u64> {
    // Baseline, restored backup, candidate, bundle and temporary repack headroom.
    // This is a conservative admission check, not a promise against disk exhaustion.
    let required = source_bytes
        .checked_mul(6)
        .and_then(|n| n.checked_add(DISK_RESERVE))
        .ok_or_else(|| CoreError::blocked("Required disk space exceeds the supported range"))?;
    if available_bytes < required {
        return Err(CoreError::blocked(format!("Insufficient free space: preparation requires at least {required} bytes; {available_bytes} bytes are available. Retained backups are never deleted automatically.")));
    }
    Ok(required)
}

pub(crate) fn preparation_space(source: &Path, destination: &Path) -> Result<(u64, u64)> {
    let objects = git_text(
        Some(source),
        &[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ],
    )?;
    let source_bytes = object_storage_bytes(Path::new(&objects))?;
    let required = check_space(source_bytes, fs2::available_space(destination)?)?;
    Ok((source_bytes, required))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn disk_admission_reserves_space_and_detects_overflow() {
        let required = 4096 * 6 + DISK_RESERVE;
        assert!(check_space(4096, required - 1).is_err());
        assert_eq!(check_space(4096, required).unwrap(), required);
        assert!(check_space(u64::MAX, u64::MAX).is_err());
    }
    #[test]
    fn object_storage_rejects_symlinks_and_oversized_files() {
        let root = tempfile::tempdir().unwrap();
        let objects = root.path().join("objects");
        fs::create_dir(&objects).unwrap();
        let file = fs::File::create(objects.join("pack")).unwrap();
        file.set_len(MAX_SOURCE_OBJECT_BYTES + 1).unwrap();
        assert!(object_storage_bytes(&objects).is_err());
        fs::remove_file(objects.join("pack")).unwrap();
        std::os::unix::fs::symlink(root.path(), objects.join("external")).unwrap();
        assert!(object_storage_bytes(&objects).is_err());
    }
}
