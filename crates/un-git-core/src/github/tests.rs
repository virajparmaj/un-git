use super::*;
use std::sync::Mutex as StdMutex;

#[derive(Default)]
struct MemoryStore(StdMutex<Option<Credentials>>);

impl CredentialStore for MemoryStore {
    fn load(&self, _: &str) -> Result<Option<Credentials>> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(&self, _: &str, value: &Credentials) -> Result<()> {
        *self.0.lock().unwrap() = Some(value.clone());
        Ok(())
    }
    fn delete(&self, _: &str) -> Result<()> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

fn client(dir: &Path) -> GithubClient {
    let mut client = GithubClient::new(dir, "Iv1.test123").unwrap();
    client.credentials = Arc::new(MemoryStore::default());
    client.credential_lock_dir = dir.join("auth-locks");
    client
}

fn account() -> GithubAccount {
    GithubAccount {
        id: 7,
        login: "fixture-user".into(),
    }
}

fn token_response() -> Value {
    json!({"access_token":"ghu_secret_fixture", "refresh_token":"ghr_secret_fixture", "expires_in":28800, "refresh_token_expires_in":15897600})
}

fn collection() -> Value {
    json!({
        "hasAnyRestrictedContributions":false,"restrictedContributionsCount":0,
        "totalCommitContributions":2,"totalIssueContributions":1,"totalPullRequestContributions":0,
        "totalPullRequestReviewContributions":0,"totalRepositoryContributions":0,
        "contributionCalendar":{"totalContributions":3,"weeks":[{"contributionDays":[
            {"date":"2024-02-29","contributionCount":3,"contributionLevel":"FIRST_QUARTILE"}
        ]}]},
        "totalRepositoriesWithContributedCommits":1,
        "commitContributionsByRepository":[{
            "repository":{"id":"R_fixture","nameWithOwner":"fixture-user/repo","isPrivate":true,"defaultBranchRef":{"name":"main"}},
            "contributions":{"totalCount":2,"pageInfo":{"hasNextPage":false,"endCursor":null},"nodes":[
                {"occurredAt":"2024-02-29T00:00:00Z","commitCount":2,"isRestricted":false}
            ]}
        }]
    })
}

fn calendar_body(collection: Value) -> Value {
    json!({"data":{"viewer":{"login":"fixture-user","years":{"contributionYears":[2024,2023]},"collection":collection}}})
}

#[test]
fn calendar_preserves_observed_counts_and_does_not_infer_shas() {
    let parsed = parse_calendar(
        &calendar_body(collection()),
        2024,
        "from".into(),
        "to".into(),
        123,
    )
    .unwrap();
    assert_eq!(parsed.days[0].date, "2024-02-29");
    assert_eq!(parsed.days[0].count, Some(3));
    assert_eq!(parsed.totals.commits, Some(2));
    assert_eq!(parsed.totals.issues, Some(1));
    assert!(!parsed.partial);
    assert_eq!(parsed.years, [2024, 2023]);
    assert!(parsed
        .coverage
        .contains("do not identify exact commit SHAs"));
}

#[test]
fn partial_graphql_keeps_surviving_data_and_redacts_remote_error_text() {
    let mut body = calendar_body(collection());
    body["errors"] =
        json!([{"type":"FORBIDDEN","message":"ghu_private_echo must never be exported"}]);
    let parsed = parse_calendar(&body, 2024, "from".into(), "to".into(), 123).unwrap();
    assert!(parsed.partial);
    assert_eq!(parsed.days[0].count, Some(3));
    assert_eq!(parsed.issues[0].code, "PERMISSION_DENIED");
    assert!(!serde_json::to_string(&parsed)
        .unwrap()
        .contains("ghu_private_echo"));
}

#[test]
fn missing_calendar_count_is_unknown_not_zero() {
    let mut value = collection();
    value["contributionCalendar"]["weeks"][0]["contributionDays"][0]["contributionCount"] =
        Value::Null;
    let parsed =
        parse_calendar(&calendar_body(value), 2024, "from".into(), "to".into(), 123).unwrap();
    assert!(parsed.partial);
    assert_eq!(parsed.days[0].count, None);
}

#[test]
fn missing_all_contribution_data_fails_explicitly() {
    let error = parse_calendar(
        &json!({"data":null,"errors":[{"type":"FORBIDDEN"}]}),
        2024,
        "from".into(),
        "to".into(),
        123,
    )
    .unwrap_err();
    assert_eq!(error.code, "CONTRIBUTIONS_UNAVAILABLE");
}

#[test]
fn full_graphql_rate_limit_retains_retryable_class_without_echoing_remote_text() {
    let body = json!({"data":null,"errors":[{
        "type":"RATE_LIMITED","message":"ghu_private_echo must never be exported"
    }]});
    let calendar_error = parse_calendar(&body, 2024, "from".into(), "to".into(), 123).unwrap_err();
    let day_error = parse_day(&body, "2024-02-29", 123).unwrap_err();
    for error in [calendar_error, day_error] {
        assert_eq!(error.code, "RATE_LIMITED");
        assert!(!serde_json::to_string(&error)
            .unwrap()
            .contains("ghu_private_echo"));
    }
}

#[test]
fn full_graphql_access_failures_never_become_cache_eligible_rate_limits() {
    for kind in [
        "FORBIDDEN",
        "INSUFFICIENT_SCOPES",
        "UNAUTHENTICATED",
        "UNKNOWN",
    ] {
        for errors in [
            json!([{"type":kind}]),
            json!([{"type":"RATE_LIMITED"},{"type":kind}]),
            json!([{"type":kind},{"type":"RATE_LIMITED"}]),
        ] {
            let body = json!({"data":null,"errors":errors});
            let calendar_error =
                parse_calendar(&body, 2024, "from".into(), "to".into(), 123).unwrap_err();
            let day_error = parse_day(&body, "2024-02-29", 123).unwrap_err();
            assert_eq!(calendar_error.code, "CONTRIBUTIONS_UNAVAILABLE");
            assert_eq!(day_error.code, "CONTRIBUTIONS_UNAVAILABLE");
        }
    }
}

#[test]
fn day_uses_reported_commit_total_not_daily_node_count() {
    let parsed = parse_day(&calendar_body(collection()), "2024-02-29", 123).unwrap();
    let repo = &parsed.repositories[0];
    assert_eq!(repo.days.len(), 1);
    assert_eq!(repo.days[0].commit_count, Some(2));
    assert_eq!(repo.observed_commit_count, Some(2));
    assert!(!repo.partial);

    let mut value = collection();
    value["commitContributionsByRepository"][0]["contributions"]["totalCount"] = Value::Null;
    let parsed = parse_day(&calendar_body(value), "2024-02-29", 123).unwrap();
    assert_eq!(parsed.repositories[0].observed_commit_count, None);
    assert_eq!(parsed.repositories[0].days[0].commit_count, Some(2));
    assert!(parsed.repositories[0].partial);
    assert!(parsed.partial);
}

#[test]
fn day_discloses_repository_cap_and_restricted_data() {
    let mut value = collection();
    value["totalRepositoriesWithContributedCommits"] = json!(150);
    value["restrictedContributionsCount"] = json!(10);
    let parsed = parse_day(&calendar_body(value), "2024-02-29", 123).unwrap();
    assert!(parsed.partial);
    assert_eq!(parsed.totals.restricted, Some(10));
    assert_eq!(parsed.repositories.len(), 1);
    assert_eq!(parsed.issues[0].code, "REPOSITORY_COVERAGE_INCOMPLETE");
}

#[test]
fn day_does_not_silently_drop_null_nodes_or_unfinished_connections() {
    let mut value = collection();
    value["commitContributionsByRepository"][0]["contributions"]["nodes"] = json!([null]);
    let parsed = parse_day(&calendar_body(value), "2024-02-29", 123).unwrap();
    assert!(parsed.partial);
    assert!(parsed.repositories[0].partial);
    let mut value = collection();
    value["commitContributionsByRepository"][0]["contributions"]["pageInfo"]["hasNextPage"] =
        json!(true);
    assert!(
        parse_day(&calendar_body(value), "2024-02-29", 123)
            .unwrap()
            .partial
    );
}

#[test]
fn year_requests_are_bounded_and_leap_year_safe() {
    let now = Utc
        .with_ymd_and_hms(2026, 9, 27, 14, 0, 0)
        .unwrap()
        .timestamp();
    assert_eq!(
        year_bounds(2024, now).unwrap(),
        ("2024-01-01T00:00:00Z".into(), "2024-12-31T23:59:59Z".into())
    );
    assert_eq!(year_bounds(2026, now).unwrap().1, "2026-09-27T14:00:00Z");
    assert!(year_bounds(2027, now).is_err());
    assert!(year_bounds(2007, now).is_err());
}

#[test]
fn repository_permissions_intersect_user_and_installation() {
    let installation = parse_installation(&json!({"id":20,"account":{"id":7,"login":"fixture-user","type":"User"},"repository_selection":"selected","permissions":{"contents":"write","pull_requests":"read","administration":"read"},"suspended_at":null})).unwrap();
    let mut value = json!({"id":30,"name":"repo","full_name":"fixture-user/repo","owner":{"id":7,"login":"fixture-user"},"private":true,"fork":false,"archived":false,"default_branch":"main","clone_url":"https://github.com/fixture-user/repo.git","permissions":{"pull":true,"push":false,"admin":false}});
    let repo = parse_repository(&value, &installation, &account()).unwrap();
    assert!(repo.contents_read);
    assert!(!repo.contents_write);
    assert!(!repo.administration_read);
    assert!(repo.personal_owner);
    value["permissions"] = Value::Null;
    let repo = parse_repository(&value, &installation, &account()).unwrap();
    assert!(!repo.contents_read);
    assert_eq!(repo.permission_status, "user_permissions_unknown");
    value["clone_url"] = json!("https://github.com@attacker.invalid/repo");
    assert!(parse_repository(&value, &installation, &account()).is_err());
}

#[test]
fn auth_http_and_oauth_failures_are_distinct() {
    assert_eq!(http_error(401, None, false, None).code, "AUTH_REVOKED");
    assert_eq!(http_error(403, None, false, None).code, "PERMISSION_DENIED");
    assert_eq!(http_error(403, None, true, Some(99)).code, "RATE_LIMITED");
    assert_eq!(
        http_error(404, None, false, None).code,
        "RESOURCE_UNAVAILABLE"
    );
    assert_eq!(
        oauth_error(&json!({"error":"device_flow_disabled"})).code,
        "DEVICE_FLOW_DISABLED"
    );
    assert_eq!(
        oauth_error(&json!({"error":"bad_refresh_token"})).code,
        "AUTH_REQUIRED"
    );
}

#[test]
fn credentials_require_expiration_and_correct_token_kind() {
    let value = token_response();
    let parsed = parse_credentials(&value, account(), 100).unwrap();
    assert_eq!(parsed.expires_at, 28900);
    let mut invalid = value.clone();
    invalid["expires_in"] = Value::Null;
    assert_eq!(
        parse_credentials(&invalid, account(), 100)
            .err()
            .unwrap()
            .code,
        "EXPIRING_TOKENS_REQUIRED"
    );
    invalid = value;
    invalid["access_token"] = json!("ghp_wrong_token_kind");
    assert!(parse_credentials(&invalid, account(), 100).is_err());
}

#[tokio::test]
async fn status_and_logout_never_return_tokens_or_write_them_to_files() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    client
        .credentials
        .save(
            &client.client_id,
            &parse_credentials(&token_response(), account(), Utc::now().timestamp()).unwrap(),
        )
        .unwrap();
    let status = client.status().await.unwrap();
    assert!(status.connected);
    let serialized = serde_json::to_string(&status).unwrap();
    assert!(!serialized.contains("secret_fixture"));
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
    client.logout().await.unwrap();
    assert!(!client.status().await.unwrap().connected);
    for entry in std::fs::read_dir(dir.path().join("auth-locks")).unwrap() {
        assert!(std::fs::read(entry.unwrap().path()).unwrap().is_empty());
    }
}

#[tokio::test]
async fn concurrent_clients_reload_rotated_credentials_under_shared_lock() {
    let dir = tempfile::tempdir().unwrap();
    let a = client(dir.path());
    let b = a.clone();
    a.credentials
        .save(
            &a.client_id,
            &parse_credentials(&token_response(), account(), Utc::now().timestamp()).unwrap(),
        )
        .unwrap();
    let (one, two) = tokio::join!(a.access_token(), b.access_token());
    assert!(one.is_ok());
    assert!(two.is_ok());
}

#[test]
fn production_credential_lock_scope_ignores_operation_data_directory() {
    let dir = tempfile::tempdir().unwrap();
    let a = GithubClient::new(dir.path().join("desktop"), "Iv1.test123").unwrap();
    let b = GithubClient::new(dir.path().join("cli"), "Iv1.test123").unwrap();
    assert_ne!(a.data_dir, b.data_dir);
    assert_eq!(a.credential_lock_dir, b.credential_lock_dir);
    assert_eq!(
        a.credential_lock_dir,
        default_credential_lock_dir().unwrap()
    );
    // Construction must not touch Keychain or create lock files in the real home.
    assert!(std::fs::read_dir(dir.path()).unwrap().next().is_none());
}

#[tokio::test]
async fn separate_data_directories_serialize_the_same_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let a = client(&dir.path().join("desktop"));
    let mut b = client(&dir.path().join("cli"));
    b.credentials = a.credentials.clone();
    b.credential_lock_dir = a.credential_lock_dir.clone();

    let held = a.credential_lock().await.unwrap();
    let next = b.credential_lock();
    tokio::pin!(next);
    assert!(tokio::time::timeout(Duration::from_millis(150), &mut next)
        .await
        .is_err());

    let mut rotated = token_response();
    rotated["access_token"] = json!("ghu_rotated_fixture");
    a.credentials
        .save(
            &a.client_id,
            &parse_credentials(&rotated, account(), Utc::now().timestamp()).unwrap(),
        )
        .unwrap();
    drop(held);
    let acquired = tokio::time::timeout(Duration::from_secs(2), &mut next)
        .await
        .unwrap()
        .unwrap();
    drop(acquired);
    assert_eq!(b.access_token().await.unwrap(), "ghu_rotated_fixture");
    assert!(!b.data_dir.exists());
}

#[cfg(unix)]
#[tokio::test]
async fn credential_lock_rejects_symlink_directory_and_file() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let client_dir = dir.path().join("client");
    std::fs::create_dir(&client_dir).unwrap();
    let client = client(&client_dir);
    symlink(&outside, &client.credential_lock_dir).unwrap();
    assert!(client.credential_lock().await.is_err());
    assert!(std::fs::read_dir(&outside).unwrap().next().is_none());

    std::fs::remove_file(&client.credential_lock_dir).unwrap();
    std::fs::create_dir(&client.credential_lock_dir).unwrap();
    let outside_file = outside.join("sentinel");
    std::fs::write(&outside_file, "preserve").unwrap();
    let filename = format!("{:x}.lock", Sha256::digest(client.client_id.as_bytes()));
    symlink(&outside_file, client.credential_lock_dir.join(filename)).unwrap();
    assert!(client.credential_lock().await.is_err());
    assert_eq!(std::fs::read_to_string(outside_file).unwrap(), "preserve");
}

#[cfg(unix)]
#[tokio::test]
async fn credential_lock_rejects_symlink_owned_parent() {
    use std::os::unix::fs::symlink;

    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir(&outside).unwrap();
    let client_dir = dir.path().join("client");
    symlink(&outside, &client_dir).unwrap();
    let client = client(&client_dir);
    assert!(client.credential_lock().await.is_err());
    assert!(std::fs::read_dir(outside).unwrap().next().is_none());
}

#[tokio::test]
async fn polling_early_is_local_and_expired_sessions_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    let client = client(dir.path());
    let now = Utc::now().timestamp();
    client.sessions.lock().await.insert(
        "session".into(),
        DeviceSession {
            device_code: "private-code".into(),
            expires_at: now + 900,
            interval_seconds: 5,
            next_poll_at: now + 60,
        },
    );
    let poll = client.poll_login("session").await.unwrap();
    assert_eq!(poll.status, "pending");
    assert!(poll.retry_after_seconds > 0);
    assert!(!serde_json::to_string(&poll)
        .unwrap()
        .contains("private-code"));
    client
        .sessions
        .lock()
        .await
        .get_mut("session")
        .unwrap()
        .expires_at = now - 1;
    assert_eq!(
        client.poll_login("session").await.unwrap_err().code,
        "DEVICE_CODE_EXPIRED"
    );
    assert!(client.sessions.lock().await.is_empty());
}

#[tokio::test]
async fn unconfigured_client_supports_offline_status_without_keychain() {
    let dir = tempfile::tempdir().unwrap();
    let client = GithubClient::new(dir.path(), "").unwrap();
    assert!(!client.status().await.unwrap().configured);
    assert_eq!(
        client.start_login().await.unwrap_err().code,
        "GITHUB_NOT_CONFIGURED"
    );
    assert!(GithubClient::new(dir.path(), "client/../../oops").is_err());
}

use std::path::Path;
