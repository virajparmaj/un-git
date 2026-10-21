use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const SCHEMA_VERSION: u32 = 1;
pub const FILTER_REPO_VERSION: &str = "2.47.0";
pub const FILTER_REPO_SHA256: &str =
    "67447413e273fc76809289111748870b6f6072f08b17efe94863a92d810b7d94";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Toolchain {
    pub git_version: String,
    pub worker_version: String,
    pub worker_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub python_version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub callback_sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Commit {
    pub oid: String,
    pub tree_oid: String,
    pub parents: Vec<String>,
    pub author_name: String,
    pub author_email: String,
    pub author_date: String,
    pub committer_name: String,
    pub committer_email: String,
    pub committer_date: String,
    pub message: String,
    /// The object payload is authoritative; display strings may be lossy UTF-8.
    pub raw_base64: String,
    pub unsupported_headers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scan {
    pub id: String,
    pub source_path: String,
    pub object_format: String,
    pub default_ref: Option<String>,
    pub refs: BTreeMap<String, String>,
    pub commits: Vec<Commit>,
    pub blockers: Vec<String>,
    pub toolchain: Toolchain,
}

pub fn default_replacement_email() -> String {
    "removed@un-git.invalid".into()
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub commit_oids: Vec<String>,
    pub confirmed_email: String,
    #[serde(default = "default_replacement_email")]
    pub replacement_email: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub id: String,
    pub content_hash: String,
    pub version: u32,
    pub scan: Scan,
    pub selection: Selection,
    pub selected_oids: Vec<String>,
    pub affected_oids: Vec<String>,
    pub blockers: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationCheck {
    pub name: String,
    pub passed: bool,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationReport {
    pub passed: bool,
    pub checks: Vec<ValidationCheck>,
    pub baseline_commit_count: usize,
    pub candidate_commit_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreparedCandidate {
    pub operation_dir: String,
    pub backup_path: String,
    pub candidate_path: String,
    pub restored_path: String,
    pub commit_map: BTreeMap<String, String>,
    pub target_refs: BTreeMap<String, String>,
    pub backup_sha256: String,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DoctorReport {
    pub ready: bool,
    pub git_version: Option<String>,
    pub python_version: Option<String>,
    pub worker_version: Option<String>,
    pub issues: Vec<String>,
}
