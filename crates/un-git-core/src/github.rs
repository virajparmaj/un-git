//! Read-only GitHub adapter. Secrets never cross the command/JSON boundary.
mod cache;
mod helper;
mod models;
mod transport;
pub use cache::SnapshotRead;
pub use helper::run_credential_helper_if_requested;
pub use models::*;
pub use transport::ImportedRepository;

use crate::{CoreError, Result};
use chrono::{Datelike, NaiveDate, TimeZone, Utc};
use fs2::FileExt;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    fs::File,
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use uuid::Uuid;

const API: &str = "https://api.github.com";
const OAUTH: &str = "https://github.com/login";
const KEYCHAIN_SERVICE: &str = "app.un-git.github-user-tokens.v1";
const RESPONSE_LIMIT: usize = 8 * 1024 * 1024;
const PAGE_LIMIT: u64 = 100;
const COVERAGE: &str = "Observed GitHub data visible to this account and app installation. Hidden private activity may be unavailable; calendar aggregates do not identify exact commit SHAs.";

const CALENDAR_QUERY: &str = r#"
query Calendar($from: DateTime!, $to: DateTime!) {
 viewer {
  login
  years: contributionsCollection { contributionYears }
  collection: contributionsCollection(from: $from, to: $to) {
   hasAnyRestrictedContributions restrictedContributionsCount
   totalCommitContributions totalIssueContributions totalPullRequestContributions
   totalPullRequestReviewContributions totalRepositoryContributions
   contributionCalendar { totalContributions weeks { contributionDays { date contributionCount contributionLevel } } }
  }
 }
}"#;

const DAY_QUERY: &str = r#"
query Day($from: DateTime!, $to: DateTime!) {
 viewer {
  collection: contributionsCollection(from: $from, to: $to) {
   restrictedContributionsCount totalCommitContributions totalIssueContributions
   totalPullRequestContributions totalPullRequestReviewContributions totalRepositoryContributions
   totalRepositoriesWithContributedCommits
   contributionCalendar { totalContributions }
   commitContributionsByRepository(maxRepositories: 100) {
    repository { id nameWithOwner isPrivate defaultBranchRef { name } }
    contributions(first: 100) {
     totalCount pageInfo { hasNextPage endCursor }
     nodes { occurredAt commitCount isRestricted }
    }
   }
  }
 }
}"#;

// Intentionally no Debug: even debug formatting of a credential must not leak it.
#[derive(Clone, Serialize, Deserialize)]
struct Credentials {
    access_token: String,
    refresh_token: String,
    expires_at: i64,
    refresh_expires_at: i64,
    account: GithubAccount,
}

trait CredentialStore: Send + Sync {
    fn load(&self, client_id: &str) -> Result<Option<Credentials>>;
    fn save(&self, client_id: &str, value: &Credentials) -> Result<()>;
    fn delete(&self, client_id: &str) -> Result<()>;
}

struct KeychainStore;

#[cfg(target_os = "macos")]
impl CredentialStore for KeychainStore {
    fn load(&self, client_id: &str) -> Result<Option<Credentials>> {
        match security_framework::passwords::get_generic_password(KEYCHAIN_SERVICE, client_id) {
            Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|_| {
                CoreError::new(
                    "KEYCHAIN_INVALID",
                    "Stored GitHub credentials are invalid; reconnect the account.",
                )
            }),
            Err(error) if error.code() == -25300 => Ok(None),
            Err(_) => Err(keychain_error()),
        }
    }
    fn save(&self, client_id: &str, value: &Credentials) -> Result<()> {
        let bytes = serde_json::to_vec(value).map_err(|_| keychain_error())?;
        // The complete rotated pair is replaced in one Keychain item.
        security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, client_id, &bytes)
            .map_err(|_| keychain_error())
    }
    fn delete(&self, client_id: &str) -> Result<()> {
        match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, client_id) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == -25300 => Ok(()),
            Err(_) => Err(keychain_error()),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl CredentialStore for KeychainStore {
    fn load(&self, _client_id: &str) -> Result<Option<Credentials>> {
        Err(keychain_error())
    }
    fn save(&self, _client_id: &str, _value: &Credentials) -> Result<()> {
        Err(keychain_error())
    }
    fn delete(&self, _client_id: &str) -> Result<()> {
        Err(keychain_error())
    }
}

fn keychain_error() -> CoreError {
    CoreError::new(
        "KEYCHAIN_UNAVAILABLE",
        "macOS Keychain access is unavailable or denied. Credentials were not written to disk.",
    )
}

struct DeviceSession {
    device_code: String,
    expires_at: i64,
    interval_seconds: u64,
    next_poll_at: i64,
}

#[derive(Clone)]
pub struct GithubClient {
    http: Client,
    client_id: String,
    data_dir: PathBuf,
    credential_lock_dir: PathBuf,
    sessions: Arc<Mutex<HashMap<String, DeviceSession>>>,
    credentials: Arc<dyn CredentialStore>,
}

impl GithubClient {
    pub fn new(data_dir: impl Into<PathBuf>, client_id: impl Into<String>) -> Result<Self> {
        let client_id = client_id.into();
        if !client_id.is_empty()
            && (client_id.len() > 128
                || !client_id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_'))
        {
            return Err(CoreError::invalid("Invalid GitHub App client ID."));
        }
        let http = Client::builder()
            .user_agent("un-git/0.1")
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| network_error())?;
        Ok(Self {
            http,
            client_id,
            data_dir: data_dir.into(),
            credential_lock_dir: default_credential_lock_dir()?,
            sessions: Arc::new(Mutex::new(HashMap::new())),
            credentials: Arc::new(KeychainStore),
        })
    }

    pub async fn status(&self) -> Result<AuthStatus> {
        let configured = !self.client_id.is_empty();
        let credentials = if configured {
            self.credentials.load(&self.client_id)?
        } else {
            None
        };
        let connected = credentials
            .as_ref()
            .is_some_and(|c| c.refresh_expires_at > Utc::now().timestamp());
        Ok(AuthStatus {
            configured,
            connected,
            account: credentials.as_ref().map(|c| c.account.clone()),
            expires_at: credentials.as_ref().map(|c| c.expires_at),
            refresh_expires_at: credentials.as_ref().map(|c| c.refresh_expires_at),
            authorization_status: if connected {
                "stored_not_verified"
            } else if credentials.is_some() {
                "reauthorization_required"
            } else {
                "disconnected"
            }
            .into(),
            installation_status: "not_checked".into(),
        })
    }

    pub async fn start_login(&self) -> Result<DeviceLogin> {
        self.require_configured()?;
        let response = self
            .http
            .post(format!("{OAUTH}/device/code"))
            .header("Accept", "application/json")
            .form(&[("client_id", self.client_id.as_str())])
            .send()
            .await
            .map_err(|_| network_error())?;
        let body = response_json(response).await?;
        if body.get("error").is_some() {
            return Err(oauth_error(&body));
        }
        let now = Utc::now().timestamp();
        let duration = body["expires_in"]
            .as_u64()
            .filter(|v| *v > 0 && *v <= 3600)
            .ok_or_else(invalid_response)?;
        let interval = body["interval"].as_u64().unwrap_or(5).clamp(5, 300);
        let device_code = required_string(&body, "device_code")?;
        let user_code = required_string(&body, "user_code")?;
        let verification_uri = required_string(&body, "verification_uri")?;
        if verification_uri != "https://github.com/login/device" {
            return Err(invalid_response());
        }
        let expires_at = now + duration as i64;
        let session_id = Uuid::new_v4().to_string();
        let mut sessions = self.sessions.lock().await;
        sessions.retain(|_, session| session.expires_at > now);
        if sessions.len() >= 4 {
            return Err(CoreError::new(
                "AUTH_BUSY",
                "Finish or cancel the pending GitHub sign-in first.",
            ));
        }
        sessions.insert(
            session_id.clone(),
            DeviceSession {
                device_code,
                expires_at,
                interval_seconds: interval,
                next_poll_at: now + interval as i64,
            },
        );
        Ok(DeviceLogin {
            session_id,
            user_code,
            verification_uri,
            expires_at,
            interval_seconds: interval,
        })
    }

    pub async fn cancel_login(&self, session_id: &str) -> Result<()> {
        self.sessions.lock().await.remove(session_id);
        Ok(())
    }

    pub async fn poll_login(&self, session_id: &str) -> Result<LoginPoll> {
        // Keep this mutex through the HTTP call to prevent duplicate polling.
        let mut sessions = self.sessions.lock().await;
        let now = Utc::now().timestamp();
        let session = sessions
            .get_mut(session_id)
            .ok_or_else(|| CoreError::new("AUTH_SESSION_MISSING", "Start GitHub sign-in again."))?;
        if now >= session.expires_at {
            sessions.remove(session_id);
            return Err(CoreError::new(
                "DEVICE_CODE_EXPIRED",
                "The sign-in code expired. Start GitHub sign-in again.",
            ));
        }
        if now < session.next_poll_at {
            return Ok(pending((session.next_poll_at - now) as u64));
        }
        session.next_poll_at = now + session.interval_seconds as i64;
        let response = self
            .http
            .post(format!("{OAUTH}/oauth/access_token"))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("device_code", session.device_code.as_str()),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ])
            .send()
            .await
            .map_err(|_| network_error())?;
        let body = response_json(response).await?;
        match body["error"].as_str() {
            Some("authorization_pending") => Ok(pending(session.interval_seconds)),
            Some("slow_down") => {
                session.interval_seconds =
                    (session.interval_seconds + 5).max(body["interval"].as_u64().unwrap_or(0));
                session.next_poll_at = Utc::now().timestamp() + session.interval_seconds as i64;
                Ok(pending(session.interval_seconds))
            }
            Some(_) => {
                sessions.remove(session_id);
                Err(oauth_error(&body))
            }
            None => {
                sessions.remove(session_id);
                let token = required_string(&body, "access_token")?;
                let account = self.account_for_token(&token).await?;
                let credentials =
                    parse_credentials(&body, account.clone(), Utc::now().timestamp())?;
                let _lock = self.credential_lock().await?;
                self.credentials.save(&self.client_id, &credentials)?;
                Ok(LoginPoll {
                    status: "connected".into(),
                    retry_after_seconds: 0,
                    account: Some(account),
                })
            }
        }
    }

    /// Local sign out only. GitHub authorization can be revoked in account settings.
    pub async fn logout(&self) -> Result<()> {
        if self.client_id.is_empty() {
            return Ok(());
        }
        let mut sessions = self.sessions.lock().await;
        let _lock = self.credential_lock().await?;
        self.credentials.delete(&self.client_id)?;
        sessions.clear();
        Ok(())
    }

    pub async fn verify_account(&self) -> Result<GithubAccount> {
        let token = self.access_token().await?;
        self.account_for_token(&token).await
    }

    pub async fn emails(&self) -> Result<Vec<AccountEmail>> {
        let token = self.access_token().await?;
        let mut emails = Vec::new();
        for page in 1..=PAGE_LIMIT {
            let value = self
                .get(&format!("/user/emails?per_page=100&page={page}"), &token)
                .await?;
            let batch: Vec<AccountEmail> =
                serde_json::from_value(value).map_err(|_| invalid_response())?;
            let done = batch.len() < 100;
            emails.extend(batch);
            if done {
                return Ok(emails);
            }
        }
        Err(CoreError::new(
            "COVERAGE_INCOMPLETE",
            "Email pagination exceeded the supported limit; ownership evidence is incomplete.",
        ))
    }

    pub async fn repositories(&self) -> Result<RepositoryList> {
        let token = self.access_token().await?;
        self.repositories_for_token(&token).await
    }

    async fn repositories_for_token(&self, token: &str) -> Result<RepositoryList> {
        let account = self.account_for_token(token).await?;
        let mut result = RepositoryList {
            fetched_at: Utc::now().timestamp(),
            installations: Vec::new(),
            repositories: Vec::new(),
            partial: false,
            issues: Vec::new(),
        };
        let installations = self
            .paginated("/user/installations", "installations", token)
            .await?;
        for value in installations {
            let installation = parse_installation(&value)?;
            if installation.suspended {
                result.partial = true;
                result.issues.push(ApiIssue {
                    code: "INSTALLATION_SUSPENDED".into(),
                    message: format!("Installation {} is suspended.", installation.id),
                });
            } else {
                let path = format!("/user/installations/{}/repositories", installation.id);
                match self.paginated(&path, "repositories", token).await {
                    Ok(repos) => {
                        for repo in repos {
                            result.repositories.push(parse_repository(
                                &repo,
                                &installation,
                                &account,
                            )?);
                        }
                    }
                    Err(error) => {
                        result.partial = true;
                        result.issues.push(ApiIssue {
                            code: error.code,
                            message: error.message,
                        });
                    }
                }
            }
            result.installations.push(installation);
        }
        result
            .repositories
            .sort_by(|a, b| a.full_name.cmp(&b.full_name));
        result.repositories.dedup_by_key(|repo| repo.id);
        Ok(result)
    }

    pub async fn calendar(&self, year: Option<i32>) -> Result<CalendarSnapshot> {
        let year = year.unwrap_or_else(|| Utc::now().year());
        let (from, to) = year_bounds(year, Utc::now().timestamp())?;
        let body = self
            .graphql(CALENDAR_QUERY, json!({"from":from,"to":to}))
            .await?;
        parse_calendar(&body, year, from, to, Utc::now().timestamp())
    }

    pub async fn day(&self, date: &str) -> Result<DaySnapshot> {
        let parsed = NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| CoreError::invalid("Use a valid YYYY-MM-DD contribution date."))?;
        if parsed.to_string() != date || parsed > Utc::now().date_naive() || parsed.year() < 2008 {
            return Err(CoreError::invalid(
                "Contribution date is outside the supported range.",
            ));
        }
        // A one-day window makes each repository's daily connection bounded by
        // one node. The repository grouping remains capped and is never assumed complete.
        let body = self
            .graphql(
                DAY_QUERY,
                json!({"from":format!("{date}T00:00:00Z"),"to":format!("{date}T23:59:59Z")}),
            )
            .await?;
        parse_day(&body, date, Utc::now().timestamp())
    }

    fn require_configured(&self) -> Result<()> {
        if self.client_id.is_empty() {
            Err(CoreError::new(
                "GITHUB_NOT_CONFIGURED",
                "Configure the public Inspector GitHub App client ID to connect an account.",
            ))
        } else {
            Ok(())
        }
    }

    async fn credential_lock(&self) -> Result<File> {
        let dir = &self.credential_lock_dir;
        let parent = dir
            .parent()
            .ok_or_else(|| CoreError::invalid("Credential lock directory has no parent."))?;
        crate::store::private_directory(parent)?;
        crate::store::private_directory(dir)?;
        let dir = std::fs::canonicalize(dir)?;
        let name = format!("{:x}.lock", Sha256::digest(self.client_id.as_bytes()));
        let file = crate::store::open_private_lock(&dir.join(name))?;
        let start = Instant::now();
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(file),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    if start.elapsed() >= Duration::from_secs(35) {
                        return Err(CoreError::new("AUTH_BUSY", "Another app or CLI process is refreshing GitHub credentials. Retry shortly."));
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }

    async fn access_token(&self) -> Result<String> {
        self.require_configured()?;
        let _lock = self.credential_lock().await?;
        // Reload under the cross-process lock: another process may have rotated.
        let previous = self
            .credentials
            .load(&self.client_id)?
            .ok_or_else(auth_required)?;
        let now = Utc::now().timestamp();
        if previous.expires_at > now + 60 {
            return Ok(previous.access_token);
        }
        if previous.refresh_expires_at <= now {
            return Err(auth_required());
        }
        let response = self
            .http
            .post(format!("{OAUTH}/oauth/access_token"))
            .header("Accept", "application/json")
            .form(&[
                ("client_id", self.client_id.as_str()),
                ("grant_type", "refresh_token"),
                ("refresh_token", previous.refresh_token.as_str()),
            ])
            .send()
            .await
            .map_err(|_| network_error())?;
        let body = response_json(response).await?;
        if body.get("error").is_some() {
            return Err(oauth_error(&body));
        }
        let next = parse_credentials(&body, previous.account, Utc::now().timestamp())?;
        self.credentials.save(&self.client_id, &next)?;
        Ok(next.access_token)
    }

    async fn account_for_token(&self, token: &str) -> Result<GithubAccount> {
        serde_json::from_value(self.get("/user", token).await?).map_err(|_| invalid_response())
    }

    async fn get(&self, path: &str, token: &str) -> Result<Value> {
        // Only module-defined paths reach this function; callers cannot supply URLs.
        let response = self
            .http
            .get(format!("{API}{path}"))
            .bearer_auth(token)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2026-03-10")
            .send()
            .await
            .map_err(|_| network_error())?;
        response_json(response).await
    }

    async fn paginated(&self, path: &str, field: &str, token: &str) -> Result<Vec<Value>> {
        let mut values = Vec::new();
        for page in 1..=PAGE_LIMIT {
            let body = self
                .get(&format!("{path}?per_page=100&page={page}"), token)
                .await?;
            let items = body[field].as_array().ok_or_else(invalid_response)?;
            let total = body["total_count"].as_u64().ok_or_else(invalid_response)?;
            values.extend(items.iter().cloned());
            if values.len() as u64 >= total {
                return Ok(values);
            }
            if items.is_empty() {
                return Err(CoreError::new("COVERAGE_INCOMPLETE", "GitHub pagination ended before its reported total; retry the repository inventory."));
            }
        }
        Err(CoreError::new(
            "COVERAGE_INCOMPLETE",
            "Repository pagination exceeded the supported limit.",
        ))
    }

    async fn graphql(&self, query: &str, variables: Value) -> Result<Value> {
        let token = self.access_token().await?;
        let response = self
            .http
            .post(format!("{API}/graphql"))
            .bearer_auth(token)
            .json(&json!({"query":query,"variables":variables}))
            .send()
            .await
            .map_err(|_| network_error())?;
        response_json(response).await
    }
}

fn default_credential_lock_dir() -> Result<PathBuf> {
    // Keychain items are keyed by client ID, not operation storage. Every real
    // client must therefore coordinate even when --data-dir differs.
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .ok_or_else(|| {
            CoreError::new(
                "AUTH_STORAGE_UNAVAILABLE",
                "An absolute user home directory is required for credential coordination.",
            )
        })?;
    #[cfg(target_os = "macos")]
    {
        Ok(home.join("Library/Application Support/com.virajparmaj.un-git/credential-locks"))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Ok(home.join(".local/share/un-git/credential-locks"))
    }
}

fn pending(seconds: u64) -> LoginPoll {
    LoginPoll {
        status: "pending".into(),
        retry_after_seconds: seconds,
        account: None,
    }
}

fn parse_credentials(value: &Value, account: GithubAccount, now: i64) -> Result<Credentials> {
    let access_token = required_string(value, "access_token")?;
    let refresh_token = required_string(value, "refresh_token")?;
    let expires = value["expires_in"]
        .as_i64()
        .filter(|v| *v > 0 && *v <= 86400)
        .ok_or_else(|| {
            CoreError::new(
                "EXPIRING_TOKENS_REQUIRED",
                "Enable expiring user access tokens in the GitHub App registration.",
            )
        })?;
    let refresh_expires = value["refresh_token_expires_in"]
        .as_i64()
        .filter(|v| *v > 0 && *v <= 366 * 86400)
        .ok_or_else(invalid_response)?;
    if !access_token.starts_with("ghu_") || !refresh_token.starts_with("ghr_") {
        return Err(invalid_response());
    }
    Ok(Credentials {
        access_token,
        refresh_token,
        expires_at: now + expires,
        refresh_expires_at: now + refresh_expires,
        account,
    })
}

async fn response_json(mut response: Response) -> Result<Value> {
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        let retry = response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        let remaining = response
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok());
        let reset = response
            .headers()
            .get("x-ratelimit-reset")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok());
        return Err(http_error(status, retry, remaining == Some("0"), reset));
    }
    if response
        .content_length()
        .is_some_and(|size| size > RESPONSE_LIMIT as u64)
    {
        return Err(CoreError::new(
            "RESPONSE_TOO_LARGE",
            "GitHub response exceeded the safety limit.",
        ));
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| network_error())? {
        if bytes.len() + chunk.len() > RESPONSE_LIMIT {
            return Err(CoreError::new(
                "RESPONSE_TOO_LARGE",
                "GitHub response exceeded the safety limit.",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| invalid_response())
}

fn http_error(status: u16, retry: Option<u64>, exhausted: bool, reset: Option<u64>) -> CoreError {
    match status {
        401 => CoreError::new("AUTH_REVOKED", "GitHub authorization is expired or revoked. Reconnect the account."),
        403 | 429 if exhausted || retry.is_some() || status == 429 => CoreError::new("RATE_LIMITED", format!("GitHub rate limit reached. Retry after {} seconds; reset timestamp {}.", retry.unwrap_or(60), reset.map(|n| n.to_string()).unwrap_or_else(|| "unknown".into()))),
        403 => CoreError::new("PERMISSION_DENIED", "GitHub denied access. Check app installation, selected repositories, and approved permissions."),
        404 => CoreError::new("RESOURCE_UNAVAILABLE", "The GitHub resource is absent or inaccessible; absence of access is not proof of deletion."),
        500..=599 => CoreError::new("GITHUB_UNAVAILABLE", "GitHub is temporarily unavailable. Retry later."),
        _ => CoreError::new("GITHUB_REQUEST_FAILED", format!("GitHub request failed with HTTP {status}.")),
    }
}

fn oauth_error(value: &Value) -> CoreError {
    match value["error"].as_str().unwrap_or("") {
        "expired_token" | "token_expired" | "bad_verification_code" => CoreError::new(
            "DEVICE_CODE_EXPIRED",
            "The GitHub sign-in code expired. Start again.",
        ),
        "access_denied" => CoreError::new("AUTH_DENIED", "GitHub sign-in was cancelled."),
        "bad_refresh_token" => auth_required(),
        "device_flow_disabled" => CoreError::new(
            "DEVICE_FLOW_DISABLED",
            "Enable device flow in the Inspector GitHub App registration.",
        ),
        "incorrect_client_credentials" => CoreError::new(
            "GITHUB_NOT_CONFIGURED",
            "The GitHub App client ID was not accepted.",
        ),
        "unverified_user_email" => CoreError::new(
            "EMAIL_NOT_VERIFIED",
            "Verify the primary email address on GitHub before signing in.",
        ),
        _ => CoreError::new(
            "GITHUB_AUTH_FAILED",
            "GitHub could not complete authentication. Start sign-in again.",
        ),
    }
}

fn auth_required() -> CoreError {
    CoreError::new(
        "AUTH_REQUIRED",
        "Connect GitHub again to continue; credentials are missing or expired.",
    )
}
fn network_error() -> CoreError {
    CoreError::new(
        "NETWORK_UNAVAILABLE",
        "Could not reach GitHub. Check the network and retry.",
    )
}
fn invalid_response() -> CoreError {
    CoreError::new(
        "GITHUB_INVALID_RESPONSE",
        "GitHub returned an incomplete or unsupported response.",
    )
}
fn required_string(value: &Value, key: &str) -> Result<String> {
    value[key]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .ok_or_else(invalid_response)
}

fn parse_installation(value: &Value) -> Result<Installation> {
    Ok(Installation {
        id: value["id"].as_u64().ok_or_else(invalid_response)?,
        account: serde_json::from_value(value["account"].clone())
            .map_err(|_| invalid_response())?,
        account_type: required_string(&value["account"], "type")?,
        repository_selection: required_string(value, "repository_selection")?,
        permissions: serde_json::from_value(value["permissions"].clone())
            .map_err(|_| invalid_response())?,
        suspended: !value["suspended_at"].is_null(),
    })
}

fn parse_repository(
    value: &Value,
    installation: &Installation,
    account: &GithubAccount,
) -> Result<GithubRepository> {
    let owner: GithubAccount =
        serde_json::from_value(value["owner"].clone()).map_err(|_| invalid_response())?;
    let full_name = required_string(value, "full_name")?;
    let clone_url = required_string(value, "clone_url")?;
    if !clone_url.starts_with("https://github.com/")
        || clone_url.contains('@')
        || clone_url.contains('?')
        || clone_url.contains('#')
    {
        return Err(invalid_response());
    }
    let can_read = |permission: &str| {
        matches!(
            installation.permissions.get(permission).map(String::as_str),
            Some("read" | "write")
        )
    };
    let user_read = value["permissions"]["pull"].as_bool() == Some(true);
    let user_write = value["permissions"]["push"].as_bool() == Some(true);
    Ok(GithubRepository {
        id: value["id"].as_u64().ok_or_else(invalid_response)?,
        installation_id: installation.id,
        name: required_string(value, "name")?,
        full_name,
        personal_owner: owner.id == account.id && installation.account_type == "User",
        owner,
        private: value["private"].as_bool().ok_or_else(invalid_response)?,
        fork: value["fork"].as_bool().ok_or_else(invalid_response)?,
        archived: value["archived"].as_bool().ok_or_else(invalid_response)?,
        default_branch: value["default_branch"].as_str().map(str::to_owned),
        clone_url,
        contents_read: user_read && can_read("contents"),
        contents_write: user_write
            && installation.permissions.get("contents").map(String::as_str) == Some("write"),
        administration_read: can_read("administration")
            && value["permissions"]["admin"].as_bool() == Some(true),
        pull_requests_read: user_read && can_read("pull_requests"),
        permission_status: if value["permissions"].is_object() {
            "intersection_observed"
        } else {
            "user_permissions_unknown"
        }
        .into(),
    })
}

fn year_bounds(year: i32, now: i64) -> Result<(String, String)> {
    let current = Utc
        .timestamp_opt(now, 0)
        .single()
        .ok_or_else(|| CoreError::invalid("Invalid observation time."))?;
    if year < 2008 || year > current.year() {
        return Err(CoreError::invalid(
            "Contribution year must be between 2008 and the current year.",
        ));
    }
    let from = format!("{year}-01-01T00:00:00Z");
    let to = if year == current.year() {
        current.format("%Y-%m-%dT%H:%M:%SZ").to_string()
    } else {
        format!("{year}-12-31T23:59:59Z")
    };
    Ok((from, to))
}

fn graphql_issues(body: &Value) -> Vec<ApiIssue> {
    body["errors"]
        .as_array()
        .map(|errors| {
            errors
                .iter()
                .map(|error| {
                    let kind = error["type"].as_str().unwrap_or("PARTIAL_GRAPHQL");
                    // Do not serialize arbitrary remote error messages: they can echo inputs.
                    let (code, message) = match kind {
                        "RATE_LIMITED" => (
                            "RATE_LIMITED",
                            "GitHub GraphQL rate limit reached; retry after the limit resets.",
                        ),
                        "FORBIDDEN" | "INSUFFICIENT_SCOPES" => (
                            "PERMISSION_DENIED",
                            "Some contribution data is not permitted for this account and app.",
                        ),
                        "NOT_FOUND" => (
                            "RESOURCE_UNAVAILABLE",
                            "Some contribution data is absent or inaccessible.",
                        ),
                        _ => (
                            "PARTIAL_GRAPHQL",
                            "GitHub returned partial contribution data.",
                        ),
                    };
                    ApiIssue {
                        code: code.into(),
                        message: message.into(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

fn contributions_unavailable(issues: &[ApiIssue], message: &str) -> CoreError {
    // GraphQL can report rate limits with HTTP 200 and no data. Preserve that
    // classification so the snapshot cache can offer its last observed result.
    // Any other error, including a denial mixed with a rate limit, must not
    // make inaccessible data look available through the cache.
    if !issues.is_empty() && issues.iter().all(|issue| issue.code == "RATE_LIMITED") {
        CoreError::new(
            "RATE_LIMITED",
            "GitHub GraphQL rate limit reached; retry after the limit resets.",
        )
    } else {
        CoreError::new("CONTRIBUTIONS_UNAVAILABLE", message)
    }
}

fn totals(value: &Value) -> ContributionTotals {
    ContributionTotals {
        commits: value["totalCommitContributions"].as_u64(),
        issues: value["totalIssueContributions"].as_u64(),
        pull_requests: value["totalPullRequestContributions"].as_u64(),
        reviews: value["totalPullRequestReviewContributions"].as_u64(),
        repositories_created: value["totalRepositoryContributions"].as_u64(),
        restricted: value["restrictedContributionsCount"].as_u64(),
    }
}

fn parse_calendar(
    body: &Value,
    year: i32,
    from: String,
    to: String,
    fetched_at: i64,
) -> Result<CalendarSnapshot> {
    let mut issues = graphql_issues(body);
    let viewer = &body["data"]["viewer"];
    let collection = &viewer["collection"];
    if collection.is_null() {
        return Err(contributions_unavailable(
            &issues,
            "GitHub did not return contribution data. Check app permissions and retry.",
        ));
    }
    let mut days = Vec::new();
    if let Some(weeks) = collection["contributionCalendar"]["weeks"].as_array() {
        for week in weeks {
            if let Some(entries) = week["contributionDays"].as_array() {
                for day in entries {
                    if let Some(date) = day["date"].as_str() {
                        days.push(ContributionDay {
                            date: date.into(),
                            count: day["contributionCount"].as_u64(),
                            level: day["contributionLevel"].as_str().map(str::to_owned),
                        });
                    } else {
                        issues.push(ApiIssue {
                            code: "MISSING_DAY".into(),
                            message: "An observed calendar day is unavailable.".into(),
                        });
                    }
                }
            } else {
                issues.push(ApiIssue {
                    code: "MISSING_WEEK".into(),
                    message: "An observed calendar week is unavailable.".into(),
                });
            }
        }
    } else {
        issues.push(ApiIssue {
            code: "MISSING_CALENDAR".into(),
            message: "The observed contribution calendar is unavailable.".into(),
        });
    }
    let years = viewer["years"]["contributionYears"]
        .as_array()
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.as_i64().and_then(|n| i32::try_from(n).ok()))
                .collect()
        })
        .unwrap_or_default();
    let observed_total = collection["contributionCalendar"]["totalContributions"].as_u64();
    Ok(CalendarSnapshot {
        account_login: required_string(viewer, "login")?,
        year,
        years,
        from,
        to,
        fetched_at,
        total_contributions: observed_total,
        partial: !issues.is_empty()
            || observed_total.is_none()
            || days.iter().any(|d| d.count.is_none()),
        days,
        totals: totals(collection),
        has_restricted: collection["hasAnyRestrictedContributions"].as_bool(),
        issues,
        coverage: COVERAGE.into(),
    })
}

fn parse_day(body: &Value, date: &str, fetched_at: i64) -> Result<DaySnapshot> {
    let mut issues = graphql_issues(body);
    let value = &body["data"]["viewer"]["collection"];
    if value.is_null() {
        return Err(contributions_unavailable(
            &issues,
            "GitHub did not return this day's contribution details.",
        ));
    }
    let mut repositories = Vec::new();
    if let Some(groups) = value["commitContributionsByRepository"].as_array() {
        for group in groups {
            let repo = &group["repository"];
            let contributions = &group["contributions"];
            let days = contributions["nodes"]
                .as_array()
                .map(|nodes| {
                    nodes
                        .iter()
                        .filter_map(|node| {
                            Some(CommitContributionDay {
                                occurred_at: node["occurredAt"].as_str()?.into(),
                                commit_count: node["commitCount"].as_u64(),
                                restricted: node["isRestricted"].as_bool().unwrap_or(true),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let partial = contributions["pageInfo"]["hasNextPage"].as_bool() != Some(false)
                || contributions["totalCount"].as_u64().is_none()
                || contributions["nodes"].as_array().is_none()
                || contributions["nodes"]
                    .as_array()
                    .is_some_and(|nodes| nodes.len() != days.len())
                || days.iter().any(|d| d.commit_count.is_none());
            repositories.push(RepositoryContribution {
                repository_node_id: required_string(repo, "id")?,
                full_name: required_string(repo, "nameWithOwner")?,
                private: repo["isPrivate"].as_bool().unwrap_or(true),
                default_branch: repo["defaultBranchRef"]["name"].as_str().map(str::to_owned),
                // This connection's totalCount counts commits, not daily nodes:
                // https://docs.github.com/en/graphql/reference/users#createdcommitcontributionconnection
                observed_commit_count: contributions["totalCount"].as_u64(),
                days,
                partial,
            });
        }
    } else {
        issues.push(ApiIssue {
            code: "REPOSITORIES_UNAVAILABLE".into(),
            message: "Repository detail for this day is unavailable.".into(),
        });
    }
    let aggregate_repository_count = value["totalRepositoriesWithContributedCommits"].as_u64();
    let incomplete_grouping =
        aggregate_repository_count.is_none_or(|count| count != repositories.len() as u64);
    if incomplete_grouping {
        issues.push(ApiIssue {
            code: "REPOSITORY_COVERAGE_INCOMPLETE".into(),
            message:
                "The bounded repository list does not establish complete coverage for this day."
                    .into(),
        });
    }
    let totals = totals(value);
    let restricted = totals.restricted.is_none_or(|n| n > 0);
    let partial = !issues.is_empty() || restricted || repositories.iter().any(|repo| repo.partial);
    Ok(DaySnapshot {
        date: date.into(),
        fetched_at,
        observed_total: value["contributionCalendar"]["totalContributions"].as_u64(),
        totals,
        repositories,
        aggregate_repository_count,
        partial,
        issues,
        coverage: COVERAGE.into(),
    })
}

#[cfg(test)]
mod tests;
