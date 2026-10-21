//! Last-observed snapshots are scoped by App and stable account ID, never by date alone.
use super::*;
use crate::store::Store;
use serde::de::DeserializeOwned;

#[derive(Serialize)]
pub struct SnapshotRead<T> {
    pub snapshot: T,
    pub stale: bool,
    pub fetch_error: Option<CoreError>,
}

fn reconcile<T: Serialize + DeserializeOwned>(
    store: &Store,
    key: &str,
    result: Result<T>,
) -> Result<SnapshotRead<T>> {
    match result {
        Ok(snapshot) => {
            store.put("github_snapshot_v1", key, &snapshot)?;
            Ok(SnapshotRead {
                snapshot,
                stale: false,
                fetch_error: None,
            })
        }
        Err(error)
            if matches!(
                error.code.as_str(),
                "NETWORK_UNAVAILABLE" | "GITHUB_UNAVAILABLE" | "RATE_LIMITED"
            ) =>
        {
            match store.get("github_snapshot_v1", key) {
                Ok(snapshot) => Ok(SnapshotRead {
                    snapshot,
                    stale: true,
                    fetch_error: Some(error),
                }),
                Err(_) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

impl GithubClient {
    async fn cache_account(&self) -> Result<GithubAccount> {
        self.status().await?.account.ok_or_else(auth_required)
    }

    async fn check_cache_account(&self, expected: &GithubAccount) -> Result<()> {
        if self.cache_account().await?.id != expected.id {
            return Err(CoreError::new(
                "ACCOUNT_CHANGED",
                "The connected GitHub account changed during this request. Reload the view.",
            ));
        }
        Ok(())
    }

    pub async fn calendar_with_cache(
        &self,
        year: Option<i32>,
    ) -> Result<SnapshotRead<CalendarSnapshot>> {
        let year = year.unwrap_or_else(|| Utc::now().year());
        year_bounds(year, Utc::now().timestamp())?;
        let account = self.cache_account().await?;
        let key = format!("{}:{}:calendar:{year}", self.client_id, account.id);
        let result = self.calendar(Some(year)).await;
        self.check_cache_account(&account).await?;
        reconcile(&Store::open(&self.data_dir)?, &key, result)
    }

    pub async fn day_with_cache(&self, date: &str) -> Result<SnapshotRead<DaySnapshot>> {
        let account = self.cache_account().await?;
        let key = format!("{}:{}:day:{date}", self.client_id, account.id);
        let result = self.day(date).await;
        self.check_cache_account(&account).await?;
        reconcile(&Store::open(&self.data_dir)?, &key, result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offline_cache_preserves_original_snapshot_and_account_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::open(dir.path()).unwrap();
        let snapshot = json!({"fetched_at":123,"days":[{"date":"2026-01-01","count":null}]});
        let fresh = reconcile(&store, "app:11:calendar:2026", Ok(snapshot.clone())).unwrap();
        assert!(!fresh.stale);
        let stale: SnapshotRead<Value> =
            reconcile(&store, "app:11:calendar:2026", Err(network_error())).unwrap();
        assert!(stale.stale);
        assert_eq!(stale.snapshot, snapshot);
        assert!(stale.fetch_error.is_some());
        assert!(reconcile::<Value>(&store, "app:22:calendar:2026", Err(network_error())).is_err());
        assert!(reconcile::<Value>(
            &store,
            "app:11:calendar:2026",
            Err(CoreError::new("AUTH_REVOKED", "Reconnect"))
        )
        .is_err());
    }
}
