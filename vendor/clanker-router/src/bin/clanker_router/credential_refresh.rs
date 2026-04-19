//! Background OAuth token refresh for the router binary.
//!
//! Proactively refreshes OAuth tokens before they expire so the proxy
//! never serves requests with stale credentials. Also supports reactive
//! refresh on 401 responses.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use clanker_router::auth::AuthStore;
use clanker_router::oauth::{self, OAuthCredentials};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

/// Refresh buffer: trigger proactive refresh this many ms before expiry.
const REFRESH_BUFFER_MS: i64 = 5 * 60 * 1000; // 5 minutes

/// Minimum sleep between refresh checks (avoid busy-loop on clock skew).
const MIN_SLEEP: Duration = Duration::from_secs(30);

/// Maximum sleep between refresh checks.
const MAX_SLEEP: Duration = Duration::from_secs(300); // 5 minutes

/// Callback that updates the in-memory provider credential after a refresh.
pub type CredentialUpdateFn = Arc<dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>> + Send + Sync>;

/// Background credential refresher for the router binary.
///
/// Reads from the router's own auth store (`~/.config/clanker-router/auth.json`),
/// refreshes proactively before expiry, and writes back with file locking.
pub struct CredentialRefresher {
    auth_path: PathBuf,
    /// Callback to update the in-memory provider after refresh.
    update_fn: CredentialUpdateFn,
    /// Notify channel for reactive refresh (triggered on 401).
    reactive_notify: Arc<Notify>,
    /// Cancellation token — exits when the proxy shuts down.
    cancel: CancellationToken,
}

impl CredentialRefresher {
    pub fn new(
        auth_path: PathBuf,
        update_fn: CredentialUpdateFn,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            auth_path,
            update_fn,
            reactive_notify: Arc::new(Notify::new()),
            cancel,
        }
    }

    /// Get a handle that can trigger reactive refresh (call on 401).
    pub fn reactive_handle(&self) -> Arc<Notify> {
        self.reactive_notify.clone()
    }

    /// Run the proactive refresh loop. Exits when cancelled.
    pub async fn run(self) {
        tracing::info!("Credential refresh loop started (auth: {})", self.auth_path.display());

        loop {
            if self.cancel.is_cancelled() {
                tracing::debug!("Credential refresh loop cancelled, exiting");
                return;
            }

            // Load current credentials and check expiry
            let sleep_duration = match self.check_and_refresh().await {
                Ok(next_check) => next_check,
                Err(e) => {
                    tracing::warn!("Credential refresh failed: {}", e);
                    Duration::from_secs(60)
                }
            };

            // Sleep until next check, wake early on reactive notify or cancel
            tokio::select! {
                () = tokio::time::sleep(sleep_duration) => {}
                () = self.reactive_notify.notified() => {
                    tracing::info!("Reactive credential refresh triggered (401)");
                }
                () = self.cancel.cancelled() => {
                    tracing::debug!("Credential refresh loop cancelled during sleep");
                    return;
                }
            }
        }
    }

    /// Check credentials and refresh if needed. Returns how long to sleep.
    async fn check_and_refresh(&self) -> Result<Duration, String> {
        let store = AuthStore::load(&self.auth_path);
        let cred = store
            .active_credential("anthropic")
            .ok_or_else(|| "no anthropic credential in auth store".to_string())?;

        let oauth_creds = OAuthCredentials::from_stored(cred)
            .ok_or_else(|| "anthropic credential is API key, no refresh needed".to_string())?;

        let now_ms = chrono::Utc::now().timestamp_millis();
        let ms_until_expiry = oauth_creds.expires - now_ms;

        // Already expired or within the refresh buffer — refresh now
        if ms_until_expiry <= REFRESH_BUFFER_MS {
            tracing::info!(
                ms_until_expiry,
                "Token {} — refreshing",
                if ms_until_expiry <= 0 { "expired" } else { "expiring soon" },
            );
            self.do_refresh(&oauth_creds.refresh).await?;
            return Ok(MIN_SLEEP);
        }

        // Not yet time to refresh
        let sleep_ms = (ms_until_expiry - REFRESH_BUFFER_MS).max(0) as u64;
        let sleep = Duration::from_millis(sleep_ms).min(MAX_SLEEP);
        tracing::debug!(
            expires_in_mins = ms_until_expiry / 60_000,
            sleep_secs = sleep.as_secs(),
            "Token still valid, sleeping",
        );
        Ok(sleep)
    }

    /// Perform the actual token refresh and persist to disk.
    async fn do_refresh(&self, refresh_token: &str) -> Result<(), String> {
        let new_creds = oauth::refresh_token(refresh_token)
            .await
            .map_err(|e| format!("OAuth refresh endpoint error: {}", e))?;

        // Persist with file locking
        save_with_lock(&self.auth_path, &new_creds)?;

        // Update the in-memory provider credential
        (self.update_fn)(new_creds.access.clone()).await;
        tracing::info!("Credential refreshed and updated in-memory");

        Ok(())
    }
}

/// Save refreshed credentials to the auth store with exclusive file locking.
fn save_with_lock(auth_path: &Path, new_creds: &OAuthCredentials) -> Result<(), String> {
    use std::io::Write;

    if let Some(parent) = auth_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {}", e))?;
    }

    let file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(auth_path)
        .map_err(|e| format!("open auth file: {}", e))?;

    // Exclusive lock (blocks if another instance is writing)
    fs4::fs_std::FileExt::lock_exclusive(&file).map_err(|e| format!("file lock: {}", e))?;

    // Re-read under lock (another process may have updated)
    let mut store = AuthStore::load(auth_path);
    store.set_credential("anthropic", "default", new_creds.to_stored());

    let json = serde_json::to_string_pretty(&store).map_err(|e| format!("serialize: {}", e))?;

    // Truncate + write
    file.set_len(0).map_err(|e| format!("truncate: {}", e))?;
    (&file).write_all(json.as_bytes()).map_err(|e| format!("write: {}", e))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(auth_path, std::fs::Permissions::from_mode(0o600));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clanker_router::auth::StoredCredential;

    #[test]
    fn save_with_lock_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "default",
            StoredCredential::OAuth {
                access_token: "old-access".into(),
                refresh_token: "old-refresh".into(),
                expires_at_ms: 1000,
                label: None,
            },
        );
        store.save(&path).unwrap();

        let new_creds = OAuthCredentials {
            access: "new-access".into(),
            refresh: "new-refresh".into(),
            expires: 9999,
        };
        save_with_lock(&path, &new_creds).unwrap();

        let reloaded = AuthStore::load(&path);
        let cred = reloaded.active_credential("anthropic").unwrap();
        match cred {
            StoredCredential::OAuth {
                access_token,
                refresh_token,
                expires_at_ms,
                ..
            } => {
                assert_eq!(access_token, "new-access");
                assert_eq!(refresh_token, "new-refresh");
                assert_eq!(*expires_at_ms, 9999);
            }
            _ => panic!("expected OAuth credential"),
        }
    }

    #[test]
    fn save_with_lock_creates_file_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("auth.json");

        let creds = OAuthCredentials {
            access: "acc".into(),
            refresh: "ref".into(),
            expires: 42,
        };
        save_with_lock(&path, &creds).unwrap();

        let store = AuthStore::load(&path);
        assert!(store.active_credential("anthropic").is_some());
    }

    #[tokio::test]
    async fn refresh_loop_exits_on_cancel() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        let cancel = CancellationToken::new();

        let update_fn: CredentialUpdateFn = Arc::new(|_token| Box::pin(async {}));
        let refresher = CredentialRefresher::new(path, update_fn, cancel.clone());

        // Cancel immediately
        cancel.cancel();

        // Should exit promptly
        tokio::time::timeout(Duration::from_secs(2), refresher.run())
            .await
            .expect("refresh loop should exit on cancel");
    }

    #[tokio::test]
    async fn refresh_loop_no_crash_without_oauth_cred() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        // Write an API key (not OAuth) — refresher should handle gracefully
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "default",
            StoredCredential::ApiKey {
                api_key: "sk-test".into(),
                label: None,
            },
        );
        store.save(&path).unwrap();

        let cancel = CancellationToken::new();
        let cancel2 = cancel.clone();
        let update_fn: CredentialUpdateFn = Arc::new(|_token| Box::pin(async {}));
        let refresher = CredentialRefresher::new(path, update_fn, cancel.clone());

        // Run briefly, then cancel
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(200)).await;
            cancel2.cancel();
        });

        tokio::time::timeout(Duration::from_secs(5), refresher.run())
            .await
            .expect("refresh loop should exit on cancel");
    }
}
