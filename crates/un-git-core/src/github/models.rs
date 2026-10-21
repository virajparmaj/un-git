use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct GithubAccount {
    pub id: u64,
    pub login: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthStatus {
    pub configured: bool,
    pub connected: bool,
    pub account: Option<GithubAccount>,
    pub expires_at: Option<i64>,
    pub refresh_expires_at: Option<i64>,
    pub authorization_status: String,
    pub installation_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeviceLogin {
    pub session_id: String,
    pub user_code: String,
    pub verification_uri: String,
    pub expires_at: i64,
    pub interval_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LoginPoll {
    pub status: String,
    pub retry_after_seconds: u64,
    pub account: Option<GithubAccount>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountEmail {
    pub email: String,
    pub verified: bool,
    pub primary: bool,
    pub visibility: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiIssue {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Installation {
    pub id: u64,
    pub account: GithubAccount,
    pub account_type: String,
    pub repository_selection: String,
    pub permissions: BTreeMap<String, String>,
    pub suspended: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GithubRepository {
    pub id: u64,
    pub installation_id: u64,
    pub name: String,
    pub full_name: String,
    pub owner: GithubAccount,
    pub private: bool,
    pub fork: bool,
    pub archived: bool,
    pub default_branch: Option<String>,
    pub clone_url: String,
    pub contents_read: bool,
    pub contents_write: bool,
    pub administration_read: bool,
    pub pull_requests_read: bool,
    pub personal_owner: bool,
    pub permission_status: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryList {
    pub fetched_at: i64,
    pub installations: Vec<Installation>,
    pub repositories: Vec<GithubRepository>,
    pub partial: bool,
    pub issues: Vec<ApiIssue>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributionDay {
    pub date: String,
    pub count: Option<u64>,
    pub level: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContributionTotals {
    pub commits: Option<u64>,
    pub issues: Option<u64>,
    pub pull_requests: Option<u64>,
    pub reviews: Option<u64>,
    pub repositories_created: Option<u64>,
    pub restricted: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarSnapshot {
    pub account_login: String,
    pub year: i32,
    pub years: Vec<i32>,
    pub from: String,
    pub to: String,
    pub fetched_at: i64,
    pub total_contributions: Option<u64>,
    pub days: Vec<ContributionDay>,
    pub totals: ContributionTotals,
    pub has_restricted: Option<bool>,
    pub partial: bool,
    pub issues: Vec<ApiIssue>,
    pub coverage: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RepositoryContribution {
    pub repository_node_id: String,
    pub full_name: String,
    pub private: bool,
    pub default_branch: Option<String>,
    pub observed_commit_count: Option<u64>,
    pub days: Vec<CommitContributionDay>,
    pub partial: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CommitContributionDay {
    pub occurred_at: String,
    pub commit_count: Option<u64>,
    pub restricted: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DaySnapshot {
    pub date: String,
    pub fetched_at: i64,
    pub observed_total: Option<u64>,
    pub totals: ContributionTotals,
    pub repositories: Vec<RepositoryContribution>,
    pub aggregate_repository_count: Option<u64>,
    pub partial: bool,
    pub issues: Vec<ApiIssue>,
    pub coverage: String,
}
