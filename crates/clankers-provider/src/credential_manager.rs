//! Credential manager with automatic OAuth token refresh.
//!
//! Handles proactive and reactive token refresh with file locking to prevent
//! race conditions when multiple clankers instances run concurrently.
//!
//! ## Refresh strategy
//!
//! Two layers, matching pi's `AuthStorage.refreshOAuthTokenWithLock()`:
//!
//! **Proactive** — A background task sleeps until 5 minutes before the
//! token's recorded expiry (which itself already has a 5-minute buffer from
//! the server's `expires_in`). When it wakes, it calls `get_credential()`
//! which triggers the refresh. This means regular requests almost never see
//! an expired token.
//!
//! **Reactive** — If a request finds the token expired (proactive refresh
//! failed or wasn't running), `get_credential()` refreshes inline. On a 401
//! from the API, `force_refresh()` ignores the in-memory expiry and goes
//! straight to refresh.
//!
//! ## Concurrency
//!
//! A `refresh_guard` mutex coalesces concurrent refresh attempts. If two
//! tasks both see an expired token, the second one waits for the first to
//! finish, then re-checks the credential — avoiding redundant HTTP calls.
//!
//! ## Fallback on failure
//!
//! When refresh fails (revoked token, network error), the manager tries
//! other configured accounts from `auth.json` before giving up. This
//! handles the case where one account's refresh token is invalidated but
//! another account still has valid credentials.
//!
//! ## File locking
//!
//! Disk writes use exclusive `fs4` file locks so multiple clankers instances
//! don't corrupt `auth.json`. Before refreshing, we read the file (lockless)
//! to check if another instance already refreshed — avoiding a redundant
//! HTTP round-trip. The lock is only held for the brief save-to-disk step.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Weak;
use std::time::Duration;

use clanker_router::auth::AuthStore;
use clanker_router::oauth::OAuthCredentials;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::auth::AuthStoreExt;
use super::auth::Credential;
use super::auth::OAuthFlow;
use crate::error::Result;

/// Manages credentials with automatic refresh for OAuth tokens.
///
/// Thread-safe — uses internal `Mutex`es so it can be shared across
/// the provider and any background tasks.
pub struct CredentialManager {
    /// Provider name (e.g. "anthropic", "openai-codex")
    provider: String,
    /// Current credential (behind a lock for interior mutability)
    credential: Mutex<Credential>,
    /// Serializes refresh attempts so concurrent callers coalesce
    refresh_guard: Mutex<()>,
    /// Path to auth.json for reading/writing refreshed tokens
    auth_path: PathBuf,
    /// Optional fallback auth path (e.g. ~/.pi/agent/auth.json)
    fallback_auth_path: Option<PathBuf>,
}

impl CredentialManager {
    /// Create a new Anthropic credential manager.
    ///
    /// Kept for backwards compatibility with existing Anthropic call sites.
    pub fn new(credential: Credential, auth_path: PathBuf, fallback_auth_path: Option<PathBuf>) -> Arc<Self> {
        Self::new_for_provider("anthropic", credential, auth_path, fallback_auth_path)
    }

    /// Create a credential manager for a specific OAuth-capable provider.
    ///
    /// For OAuth credentials, automatically starts a background task that
    /// proactively refreshes the token before it expires. The task uses a
    /// `Weak` reference, so dropping all strong `Arc` refs stops it.
    pub fn new_for_provider(
        provider: impl Into<String>,
        credential: Credential,
        auth_path: PathBuf,
        fallback_auth_path: Option<PathBuf>,
    ) -> Arc<Self> {
        let is_oauth = credential.is_oauth();
        let mgr = Arc::new(Self {
            provider: provider.into(),
            credential: Mutex::new(credential),
            refresh_guard: Mutex::new(()),
            auth_path,
            fallback_auth_path,
        });

        if is_oauth {
            tokio::spawn(proactive_refresh_loop(Arc::downgrade(&mgr)));
        }

        mgr
    }

    /// Get the current credential, refreshing if expired.
    ///
    /// For API keys, returns immediately. For OAuth tokens, checks expiry
    /// and refreshes inline if needed.
    pub async fn get_credential(&self) -> Result<Credential> {
        let cred = self.credential.lock().await;
        if !cred.is_expired() {
            return Ok(cred.clone());
        }
        let refresh_token = match cred.refresh_token() {
            Some(rt) => rt.to_string(),
            None => return Ok(cred.clone()), // API keys don't expire
        };
        drop(cred);

        info!("OAuth token expired, refreshing...");
        self.do_refresh(&refresh_token).await
    }

    /// Force a refresh (called reactively on 401 errors).
    ///
    /// Ignores the in-memory expiry — the server said our token is bad.
    pub async fn force_refresh(&self) -> Result<Credential> {
        let cred = self.credential.lock().await;
        let refresh_token = match cred.refresh_token() {
            Some(rt) => rt.to_string(),
            None => {
                return Err(crate::error::auth_err("Cannot refresh a non-OAuth credential"));
            }
        };
        drop(cred);

        info!("Forcing OAuth token refresh (401 received)");
        self.do_refresh(&refresh_token).await
    }

    /// Perform the actual refresh, coalescing concurrent attempts.
    ///
    /// 1. Acquire refresh guard (serializes concurrent callers)
    /// 2. Re-check credential (another caller may have already refreshed)
    /// 3. Check disk (lockless) — another process may have refreshed
    /// 4. HTTP refresh (async, no file lock held)
    /// 5. Save to disk (spawn_blocking, brief exclusive file lock)
    /// 6. Update in-memory credential
    /// 7. On failure, try other configured accounts
    async fn do_refresh(&self, refresh_token: &str) -> Result<Credential> {
        // 1. Coalesce concurrent refresh attempts
        let _guard = self.refresh_guard.lock().await;

        // 2. Re-check — another concurrent caller may have finished while we waited
        {
            let cred = self.credential.lock().await;
            if !cred.is_expired() {
                debug!("credential already refreshed by concurrent caller");
                return Ok(cred.clone());
            }
        }

        // 3. Quick disk check — skip HTTP if another process already refreshed
        let store = {
            let path = self.auth_path.clone();
            tokio::task::spawn_blocking(move || AuthStore::load(&path))
                .await
                .map_err(|e| crate::error::auth_err(format!("Disk read panicked: {e}")))?
        };
        if let Some(cred) = store.active_credential(&self.provider)
            && !cred.is_expired()
        {
            info!("{} token already refreshed by another instance", self.provider);
            let fresh = cred.clone();
            *self.credential.lock().await = fresh.clone();
            return Ok(fresh);
        }

        let oauth_flow = OAuthFlow::from_provider(Some(&self.provider))?;

        // 4. HTTP refresh (no lock held — this can take hundreds of ms)
        match oauth_flow.refresh_token(refresh_token).await {
            Ok(new_creds) => {
                // 5. Save to disk with file locking
                let creds_clone = new_creds.clone();
                let path = self.auth_path.clone();
                let provider = self.provider.clone();
                tokio::task::spawn_blocking(move || save_with_file_lock_for_provider(&path, &provider, &creds_clone))
                    .await
                    .map_err(|e| crate::error::auth_err(format!("Save task panicked: {e}")))??;

                info!("{} OAuth token refreshed, new expiry: {}", self.provider, new_creds.expires);
                crate::openai_codex::reset_entitlement(&self.provider, None);

                // 6. Update in-memory
                let new_credential = new_creds.to_stored();
                *self.credential.lock().await = new_credential.clone();

                Ok(new_credential)
            }
            Err(refresh_err) => {
                // 7. Refresh failed — try falling back to another account
                warn!("OAuth refresh failed: {refresh_err}");
                self.try_fallback_account(&store)
                    .await
                    .ok_or_else(|| crate::error::auth_err(format!("OAuth refresh failed and no fallback accounts available: {refresh_err}")))
            }
        }
    }

    /// Try to find another configured account with a valid (non-expired) credential.
    ///
    /// When the active account's refresh token is revoked or the refresh
    /// endpoint is down, this lets us fall back to a different account that
    /// still has a valid token.
    async fn try_fallback_account(&self, store: &AuthStore) -> Option<Credential> {
        let active = store.active_account_name_for(&self.provider).to_string();

        for (name, cred) in store.all_credentials(&self.provider) {
            if name != active && !cred.is_expired() {
                info!("falling back to account '{name}' after refresh failure");
                *self.credential.lock().await = cred.clone();
                return Some(cred);
            }
        }

        None
    }

    /// Whether the current credential is OAuth.
    pub async fn is_oauth(&self) -> bool {
        self.credential.lock().await.is_oauth()
    }

    /// Reload credentials from the auth store on disk.
    ///
    /// Called after `/login` completes to pick up the freshly-saved tokens
    /// without going through the OAuth refresh endpoint.
    pub async fn reload_from_disk(&self) {
        let store = AuthStore::load(&self.auth_path);
        if let Some(creds) = store.active_oauth_credentials_for(&self.provider)
            && !creds.is_expired()
        {
            info!("Reloaded {} credentials from disk after login", self.provider);
            crate::openai_codex::reset_entitlement(&self.provider, None);
            *self.credential.lock().await = creds.to_stored();
            return;
        }

        // Try fallback path (e.g. ~/.pi/agent/auth.json)
        if let Some(ref fallback) = self.fallback_auth_path {
            let store = AuthStore::load(fallback);
            if let Some(creds) = store.active_oauth_credentials_for(&self.provider)
                && !creds.is_expired()
            {
                info!("Reloaded {} credentials from fallback auth path", self.provider);
                crate::openai_codex::reset_entitlement(&self.provider, None);
                *self.credential.lock().await = creds.to_stored();
            }
        }
    }

    /// Directly update the in-memory credential (e.g. after a fresh login).
    pub async fn set_credential(&self, credential: Credential) {
        crate::openai_codex::reset_entitlement(&self.provider, None);
        *self.credential.lock().await = credential;
    }

    /// Get the current token string (without refresh check).
    pub async fn token(&self) -> String {
        self.credential.lock().await.token().to_string()
    }
}

// ── Proactive refresh ───────────────────────────────────────────────────

/// Background loop that refreshes the OAuth token before it expires.
///
/// Uses a `Weak` reference so the loop exits when the `CredentialManager`
/// is dropped (no Arc cycle).
#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; bounded by channel close"))]
async fn proactive_refresh_loop(weak: Weak<CredentialManager>) {
    loop {
        let mgr = match weak.upgrade() {
            Some(m) => m,
            None => return, // CredentialManager dropped
        };

        let sleep_dur = {
            let cred = mgr.credential.lock().await;
            match &*cred {
                Credential::OAuth { expires_at_ms, .. } => {
                    let now_ms = chrono::Utc::now().timestamp_millis();
                    // Wake 5 minutes before our recorded expiry. Since the
                    // recorded expiry already has a 5-min buffer from the
                    // server's expires_in, this fires ~10 min before real
                    // server expiry and ~5 min before is_expired() returns
                    // true.
                    let refresh_at_ms = expires_at_ms - (5 * 60 * 1000);
                    let delay_ms = (refresh_at_ms - now_ms).max(0) as u64;
                    Duration::from_millis(delay_ms)
                }
                Credential::ApiKey { .. } => return,
            }
        };

        // Drop the Arc during sleep so we don't keep the manager alive
        drop(mgr);

        if sleep_dur.is_zero() {
            // Token already needs refresh
            if let Some(mgr) = weak.upgrade() {
                info!("proactive OAuth refresh (token about to expire)");
                if let Err(e) = mgr.get_credential().await {
                    warn!("proactive refresh failed: {e}");
                }
            }
            // Backoff before retrying to avoid a tight loop on persistent failures
            tokio::time::sleep(Duration::from_secs(60)).await;
        } else {
            debug!("proactive refresh scheduled in {}s", sleep_dur.as_secs());
            tokio::time::sleep(sleep_dur).await;

            if let Some(mgr) = weak.upgrade() {
                info!("proactive OAuth refresh (scheduled)");
                if let Err(e) = mgr.get_credential().await {
                    warn!("proactive refresh failed: {e}");
                    // Retry after a backoff rather than immediately
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    }
}

// ── File-locked disk save ───────────────────────────────────────────────

/// Save refreshed Anthropic credentials to disk under an exclusive file lock.
///
/// Backwards-compatible wrapper for tests that still exercise the Anthropic path.
#[cfg(test)]
fn save_with_file_lock(auth_path: &std::path::Path, creds: &OAuthCredentials) -> Result<()> {
    save_with_file_lock_for_provider(auth_path, "anthropic", creds)
}

/// Save refreshed credentials to disk under an exclusive file lock.
///
/// Runs in `spawn_blocking` because `fs4` lock operations are blocking.
/// The lock is held only for the read-modify-write cycle (~ms), not during
/// any network calls.
fn save_with_file_lock_for_provider(auth_path: &std::path::Path, provider: &str, creds: &OAuthCredentials) -> Result<()> {
    use std::fs;
    use std::io::Write;

    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent).ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(parent, fs::Permissions::from_mode(0o700)).ok();
        }
    }
    if !auth_path.exists() {
        let mut f =
            fs::File::create(auth_path).map_err(|e| crate::error::auth_err(format!("Create auth file: {e}")))?;
        f.write_all(b"{}").ok();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(auth_path, fs::Permissions::from_mode(0o600)).ok();
        }
    }

    let lock_file =
        fs::File::open(auth_path).map_err(|e| crate::error::auth_err(format!("Open auth file for locking: {e}")))?;

    let mut is_locked = false;
    for attempt in 0..30 {
        match fs4::fs_std::FileExt::try_lock_exclusive(&lock_file) {
            Ok(true) => {
                is_locked = true;
                break;
            }
            Ok(false) | Err(_) => {
                if attempt < 29 {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    if !is_locked {
        warn!("Could not acquire auth file lock after 30s, proceeding without lock");
    }

    let _guard = UnlockGuard {
        locked: is_locked,
        file: &lock_file,
    };

    // Read-modify-write under lock
    let mut store = AuthStore::load(auth_path);
    let account_name = store.active_account_name_for(provider).to_string();
    store.set_provider_credentials(provider, &account_name, creds.clone());
    store
        .save(auth_path)
        .map_err(|e| crate::error::auth_err(format!("Save refreshed credentials: {e}")))?;

    Ok(())
}

/// RAII guard that releases the file lock on drop.
struct UnlockGuard<'a> {
    locked: bool,
    file: &'a std::fs::File,
}

impl Drop for UnlockGuard<'_> {
    fn drop(&mut self) {
        if self.locked {
            fs4::fs_std::FileExt::unlock(self.file).ok();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthStoreExt;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // ── Helpers ─────────────────────────────────────────────────────

    fn api_key_credential() -> Credential {
        Credential::ApiKey {
            api_key: "sk-test-key".into(),
            label: None,
        }
    }

    fn fresh_oauth_credential() -> Credential {
        Credential::OAuth {
            access_token: "fresh-token".into(),
            refresh_token: "refresh-tok".into(),
            expires_at_ms: chrono::Utc::now().timestamp_millis() + 3_600_000, // +1h
            label: None,
        }
    }

    fn expired_oauth_credential() -> Credential {
        Credential::OAuth {
            access_token: "expired-token".into(),
            refresh_token: "refresh-tok".into(),
            expires_at_ms: 0, // long expired
            label: None,
        }
    }

    fn temp_auth_path() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        (dir, path)
    }

    /// Create a CredentialManager without spawning the proactive refresh loop.
    /// (Avoids background tasks that call real oauth::refresh_token.)
    fn make_manager(credential: Credential, auth_path: PathBuf, fallback: Option<PathBuf>) -> Arc<CredentialManager> {
        Arc::new(CredentialManager {
            provider: "anthropic".to_string(),
            credential: Mutex::new(credential),
            refresh_guard: Mutex::new(()),
            auth_path,
            fallback_auth_path: fallback,
        })
    }

    // ── API key (no refresh needed) ─────────────────────────────────

    #[tokio::test]
    async fn api_key_returns_immediately() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);

        let cred = mgr.get_credential().await.unwrap();
        assert_eq!(cred.token(), "sk-test-key");
        assert!(!cred.is_oauth());
    }

    #[tokio::test]
    async fn api_key_never_expires() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);

        // Call get_credential many times — should always return the same key
        for _ in 0..10 {
            let cred = mgr.get_credential().await.unwrap();
            assert_eq!(cred.token(), "sk-test-key");
        }
    }

    #[tokio::test]
    async fn api_key_force_refresh_fails() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);

        let err = mgr.force_refresh().await.unwrap_err();
        assert!(err.message.contains("non-OAuth"), "got: {}", err.message);
    }

    #[tokio::test]
    async fn api_key_is_not_oauth() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);
        assert!(!mgr.is_oauth().await);
    }

    // ── Fresh OAuth (not expired) ───────────────────────────────────

    #[tokio::test]
    async fn fresh_oauth_returns_without_refresh() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(fresh_oauth_credential(), path, None);

        let cred = mgr.get_credential().await.unwrap();
        assert_eq!(cred.token(), "fresh-token");
        assert!(cred.is_oauth());
    }

    #[tokio::test]
    async fn fresh_oauth_is_oauth() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(fresh_oauth_credential(), path, None);
        assert!(mgr.is_oauth().await);
    }

    // ── Token getter ────────────────────────────────────────────────

    #[tokio::test]
    async fn token_returns_current_value() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);
        assert_eq!(mgr.token().await, "sk-test-key");
    }

    // ── set_credential ──────────────────────────────────────────────

    #[tokio::test]
    async fn set_credential_replaces_in_memory() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);

        assert_eq!(mgr.token().await, "sk-test-key");

        let new_cred = Credential::ApiKey {
            api_key: "sk-new-key".into(),
            label: None,
        };
        mgr.set_credential(new_cred).await;

        assert_eq!(mgr.token().await, "sk-new-key");
    }

    #[tokio::test]
    async fn set_credential_switches_key_to_oauth() {
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(api_key_credential(), path, None);

        assert!(!mgr.is_oauth().await);

        mgr.set_credential(fresh_oauth_credential()).await;
        assert!(mgr.is_oauth().await);
        assert_eq!(mgr.token().await, "fresh-token");
    }

    // ── reload_from_disk ────────────────────────────────────────────

    #[tokio::test]
    async fn reload_picks_up_disk_credentials() {
        let (_dir, path) = temp_auth_path();

        // Write fresh credentials to disk
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "disk-token".into(),
            refresh: "disk-refresh".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        };
        store.set_credentials("default", creds);
        store.save(&path).unwrap();

        // Manager starts with a stale credential
        let mgr = make_manager(api_key_credential(), path, None);
        assert_eq!(mgr.token().await, "sk-test-key");

        // After reload, should pick up the disk credential
        mgr.reload_from_disk().await;
        assert_eq!(mgr.token().await, "disk-token");
    }

    #[tokio::test]
    async fn reload_ignores_expired_disk_credentials() {
        let (_dir, path) = temp_auth_path();

        // Write expired credentials to disk
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "expired-disk-token".into(),
            refresh: "r".into(),
            expires: 0, // expired
        };
        store.set_credentials("default", creds);
        store.save(&path).unwrap();

        let mgr = make_manager(api_key_credential(), path, None);
        mgr.reload_from_disk().await;

        // Should NOT have picked up the expired credential
        assert_eq!(mgr.token().await, "sk-test-key");
    }

    #[tokio::test]
    async fn reload_falls_back_to_fallback_path() {
        let (_dir, primary_path) = temp_auth_path();
        let (_dir2, fallback_path) = temp_auth_path();

        // Primary path: no credentials (or expired)
        std::fs::write(&primary_path, "{}").ok();

        // Fallback path: fresh credentials
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "fallback-token".into(),
            refresh: "r".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        };
        store.set_credentials("default", creds);
        store.save(&fallback_path).unwrap();

        let mgr = make_manager(api_key_credential(), primary_path, Some(fallback_path));
        mgr.reload_from_disk().await;

        assert_eq!(mgr.token().await, "fallback-token");
    }

    #[tokio::test]
    async fn reload_no_op_when_no_file_exists() {
        let (_dir, path) = temp_auth_path();
        // Don't create the file — AuthStore::load returns default
        let mgr = make_manager(api_key_credential(), path, None);
        mgr.reload_from_disk().await;

        // Should keep original credential
        assert_eq!(mgr.token().await, "sk-test-key");
    }

    // ── save_with_file_lock ─────────────────────────────────────────

    #[test]
    fn save_creates_file_if_missing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("subdir").join("auth.json");

        let creds = OAuthCredentials {
            access: "new-tok".into(),
            refresh: "new-ref".into(),
            expires: i64::MAX,
        };

        save_with_file_lock(&path, &creds).unwrap();

        // File should exist and contain the credential
        let store = AuthStore::load(&path);
        let saved = store.active_credential("anthropic").unwrap();
        assert_eq!(saved.token(), "new-tok");
    }

    #[test]
    fn save_preserves_other_accounts() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        // Pre-populate with two accounts
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "primary",
            Credential::ApiKey {
                api_key: "sk-primary".into(),
                label: None,
            },
        );
        store.set_credential(
            "anthropic",
            "backup",
            Credential::ApiKey {
                api_key: "sk-backup".into(),
                label: None,
            },
        );
        store.save(&path).unwrap();

        // save_with_file_lock updates the active account only
        let creds = OAuthCredentials {
            access: "oauth-primary".into(),
            refresh: "r".into(),
            expires: i64::MAX,
        };
        save_with_file_lock(&path, &creds).unwrap();

        // Backup should still be there
        let store = AuthStore::load(&path);
        let backup = store.credential_for("anthropic", "backup").unwrap();
        assert_eq!(backup.token(), "sk-backup");
    }

    #[test]
    fn save_concurrent_writers_dont_corrupt() {
        // Multiple threads saving different credentials — file should never be corrupt
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, "{}").ok();

        let num_threads = 4;
        let writes_per_thread = 5;
        let barrier = Arc::new(std::sync::Barrier::new(num_threads));

        let handles: Vec<_> = (0..num_threads)
            .map(|i| {
                let path = path.clone();
                let barrier = barrier.clone();
                std::thread::spawn(move || {
                    barrier.wait(); // all start together
                    for j in 0..writes_per_thread {
                        let creds = OAuthCredentials {
                            access: format!("tok-{i}-{j}"),
                            refresh: format!("ref-{i}-{j}"),
                            expires: i64::MAX,
                        };
                        save_with_file_lock(&path, &creds).unwrap();
                    }
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        // File should be valid JSON with exactly one active credential
        let store = AuthStore::load(&path);
        let cred = store.active_credential("anthropic").unwrap();
        assert!(cred.token().starts_with("tok-"));
    }

    // ── Expired OAuth: disk check shortcut ──────────────────────────

    #[tokio::test]
    async fn expired_oauth_picks_up_disk_refresh() {
        let (_dir, path) = temp_auth_path();

        // Simulate another instance having already refreshed the token on disk
        let mut store = AuthStore::default();
        let fresh_creds = OAuthCredentials {
            access: "refreshed-by-other-instance".into(),
            refresh: "r".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        };
        store.set_credentials("default", fresh_creds);
        store.save(&path).unwrap();

        // Manager has an expired token in memory
        let mgr = make_manager(expired_oauth_credential(), path, None);

        // get_credential should find the fresh token on disk (step 3 in do_refresh)
        // and return it without calling oauth::refresh_token
        let cred = mgr.get_credential().await.unwrap();
        assert_eq!(cred.token(), "refreshed-by-other-instance");

        // In-memory credential should also be updated
        assert_eq!(mgr.token().await, "refreshed-by-other-instance");
    }

    // ── Fallback account selection ──────────────────────────────────

    #[tokio::test]
    async fn try_fallback_finds_healthy_account() {
        let (_dir, path) = temp_auth_path();

        // Set up a store with two accounts: "default" (expired) and "backup" (fresh)
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "default",
            Credential::OAuth {
                access_token: "expired".into(),
                refresh_token: "r".into(),
                expires_at_ms: 0,
                label: None,
            },
        );
        store.set_credential(
            "anthropic",
            "backup",
            Credential::OAuth {
                access_token: "backup-token".into(),
                refresh_token: "r2".into(),
                expires_at_ms: chrono::Utc::now().timestamp_millis() + 3_600_000,
                label: None,
            },
        );
        // "default" is active
        store.switch_account("anthropic", "default");
        store.save(&path).unwrap();

        let mgr = make_manager(expired_oauth_credential(), path, None);

        let fallback = mgr.try_fallback_account(&store).await;
        assert!(fallback.is_some());
        assert_eq!(fallback.unwrap().token(), "backup-token");

        // In-memory credential should have been updated to the fallback
        assert_eq!(mgr.token().await, "backup-token");
    }

    #[tokio::test]
    async fn try_fallback_skips_expired_accounts() {
        let (_dir, path) = temp_auth_path();

        // All accounts expired
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "default",
            Credential::OAuth {
                access_token: "expired1".into(),
                refresh_token: "r".into(),
                expires_at_ms: 0,
                label: None,
            },
        );
        store.set_credential(
            "anthropic",
            "other",
            Credential::OAuth {
                access_token: "expired2".into(),
                refresh_token: "r".into(),
                expires_at_ms: 0,
                label: None,
            },
        );
        store.switch_account("anthropic", "default");

        let mgr = make_manager(expired_oauth_credential(), path, None);

        let fallback = mgr.try_fallback_account(&store).await;
        assert!(fallback.is_none());
    }

    #[tokio::test]
    async fn try_fallback_skips_active_account() {
        let (_dir, path) = temp_auth_path();

        // Only the active account is fresh — should return None (can't fall back to self)
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "default",
            Credential::OAuth {
                access_token: "active-fresh".into(),
                refresh_token: "r".into(),
                expires_at_ms: chrono::Utc::now().timestamp_millis() + 3_600_000,
                label: None,
            },
        );
        store.switch_account("anthropic", "default");

        let mgr = make_manager(expired_oauth_credential(), path, None);

        let fallback = mgr.try_fallback_account(&store).await;
        assert!(fallback.is_none());
    }

    // ── Concurrent refresh coalescing ───────────────────────────────

    #[tokio::test]
    async fn concurrent_get_credential_coalesces() {
        let (_dir, path) = temp_auth_path();

        // Put fresh credentials on disk so the "disk check" path succeeds
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "disk-refreshed".into(),
            refresh: "r".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        };
        store.set_credentials("default", creds);
        store.save(&path).unwrap();

        let mgr = make_manager(expired_oauth_credential(), path, None);

        // Spawn multiple concurrent get_credential calls — all should succeed
        // and return the same disk-refreshed token
        let call_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..10 {
            let mgr = mgr.clone();
            let count = call_count.clone();
            handles.push(tokio::spawn(async move {
                let cred = mgr.get_credential().await.unwrap();
                count.fetch_add(1, Ordering::SeqCst);
                cred.token().to_string()
            }));
        }

        let mut tokens = Vec::new();
        for h in handles {
            tokens.push(h.await.unwrap());
        }

        // All calls completed
        assert_eq!(call_count.load(Ordering::SeqCst), 10);
        // All got the same token
        for t in &tokens {
            assert_eq!(t, "disk-refreshed");
        }
    }

    #[tokio::test]
    async fn second_caller_sees_first_callers_refresh() {
        let (_dir, path) = temp_auth_path();

        // Put fresh creds on disk — first caller will pick them up via disk check
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "first-refresh".into(),
            refresh: "r".into(),
            expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
        };
        store.set_credentials("default", creds);
        store.save(&path).unwrap();

        let mgr = make_manager(expired_oauth_credential(), path, None);

        // First call triggers do_refresh → disk check succeeds
        let cred1 = mgr.get_credential().await.unwrap();
        assert_eq!(cred1.token(), "first-refresh");

        // Second call should see the now-fresh in-memory credential (no refresh needed)
        let cred2 = mgr.get_credential().await.unwrap();
        assert_eq!(cred2.token(), "first-refresh");
    }

    // ── CredentialManager::new spawns proactive refresh for OAuth ───

    #[tokio::test]
    async fn new_with_api_key_no_background_task() {
        let (_dir, path) = temp_auth_path();
        let mgr = CredentialManager::new(api_key_credential(), path, None);

        // Should work fine — no proactive refresh spawned for API keys
        let cred = mgr.get_credential().await.unwrap();
        assert_eq!(cred.token(), "sk-test-key");
    }

    #[tokio::test]
    async fn new_with_oauth_spawns_refresh_task() {
        let (_dir, path) = temp_auth_path();
        // Use fresh OAuth so the proactive loop just sleeps (doesn't try HTTP)
        let mgr = CredentialManager::new(fresh_oauth_credential(), path, None);

        let cred = mgr.get_credential().await.unwrap();
        assert_eq!(cred.token(), "fresh-token");

        // Drop the Arc — proactive refresh loop should exit via Weak::upgrade() returning None
        drop(mgr);
        // Give the background task a moment to notice the drop
        tokio::time::sleep(Duration::from_millis(50)).await;
        // No panic = success. The loop checked Weak::upgrade() and exited.
    }

    // ── Proactive refresh loop: Weak ref cleanup ────────────────────

    #[tokio::test]
    async fn proactive_loop_exits_when_manager_dropped() {
        let (_dir, path) = temp_auth_path();
        let mgr = CredentialManager::new(fresh_oauth_credential(), path, None);

        // The loop is running. Drop the only strong ref.
        drop(mgr);

        // Give the loop time to wake and see the Weak is dead
        tokio::time::sleep(Duration::from_millis(100)).await;
        // If we get here without hanging, the loop exited cleanly
    }

    // ── Edge cases ──────────────────────────────────────────────────

    #[tokio::test]
    async fn get_credential_with_missing_auth_file() {
        // Auth file doesn't exist — disk check should still work (AuthStore::load returns default)
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nonexistent").join("auth.json");

        // Manager has expired OAuth — disk check will find nothing, then try oauth::refresh_token
        // which will fail (no real server). But the disk load shouldn't panic.
        let mgr = make_manager(expired_oauth_credential(), path, None);

        // This will fail because:
        // 1. Disk has no fresh creds
        // 2. oauth::refresh_token fails (no real server)
        // 3. No fallback accounts
        // The important thing is it doesn't panic.
        let result = mgr.get_credential().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn force_refresh_on_fresh_cred_returns_ok() {
        // force_refresh → do_refresh → step 2 re-checks in-memory cred →
        // finds it fresh → returns Ok (coalescing shortcut)
        let (_dir, path) = temp_auth_path();
        let mgr = make_manager(fresh_oauth_credential(), path, None);

        let cred = mgr.force_refresh().await.unwrap();
        assert_eq!(cred.token(), "fresh-token");
    }

    #[tokio::test]
    async fn force_refresh_on_expired_cred_tries_http() {
        // Expired in-memory + no disk creds + no real OAuth server = error
        let (_dir, path) = temp_auth_path();
        std::fs::write(&path, "{}").ok();
        let mgr = make_manager(expired_oauth_credential(), path, None);

        let result = mgr.force_refresh().await;
        assert!(result.is_err());
    }

    // ── save_with_file_lock edge cases ──────────────────────────────

    #[test]
    fn save_to_existing_empty_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");
        std::fs::write(&path, "{}").ok();

        let creds = OAuthCredentials {
            access: "tok".into(),
            refresh: "ref".into(),
            expires: i64::MAX,
        };

        save_with_file_lock(&path, &creds).unwrap();

        let store = AuthStore::load(&path);
        let saved = store.active_credential("anthropic").unwrap();
        assert_eq!(saved.token(), "tok");
    }

    #[test]
    fn save_updates_active_account_only() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        // Set up store with "work" as active
        let mut store = AuthStore::default();
        store.set_credential(
            "anthropic",
            "work",
            Credential::ApiKey {
                api_key: "old-work-key".into(),
                label: None,
            },
        );
        store.set_credential(
            "anthropic",
            "personal",
            Credential::ApiKey {
                api_key: "personal-key".into(),
                label: None,
            },
        );
        store.switch_account("anthropic", "work");
        store.save(&path).unwrap();

        let creds = OAuthCredentials {
            access: "new-work-token".into(),
            refresh: "r".into(),
            expires: i64::MAX,
        };

        save_with_file_lock(&path, &creds).unwrap();

        let store = AuthStore::load(&path);
        // Active account (work) was updated
        let work = store.credential_for("anthropic", "work").unwrap();
        assert_eq!(work.token(), "new-work-token");
        // Personal was not touched
        let personal = store.credential_for("anthropic", "personal").unwrap();
        assert_eq!(personal.token(), "personal-key");
    }

    #[test]
    fn save_read_modify_write_is_atomic() {
        // Verify that save reads the existing file, merges, and writes back
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        // Pre-populate with a non-anthropic provider
        let mut store = AuthStore::default();
        store.set_credential(
            "openai",
            "default",
            Credential::ApiKey {
                api_key: "sk-openai".into(),
                label: None,
            },
        );
        store.save(&path).unwrap();

        // save_with_file_lock should preserve the openai credential
        let creds = OAuthCredentials {
            access: "anthropic-tok".into(),
            refresh: "r".into(),
            expires: i64::MAX,
        };
        save_with_file_lock(&path, &creds).unwrap();

        let store = AuthStore::load(&path);
        // Anthropic was added
        assert!(store.active_credential("anthropic").is_some());
        // OpenAI was preserved
        let openai = store.credential_for("openai", "default").unwrap();
        assert_eq!(openai.token(), "sk-openai");
    }

    // ── File permission hardening ───────────────────────────────────

    #[cfg(unix)]
    #[test]
    fn save_with_file_lock_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().unwrap();
        let subdir = dir.path().join("secure");
        let path = subdir.join("auth.json");

        let creds = OAuthCredentials {
            access: "tok".into(),
            refresh: "ref".into(),
            expires: i64::MAX,
        };

        save_with_file_lock(&path, &creds).unwrap();

        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "auth file should be 0600, got {:#o}", file_mode);

        let dir_mode = std::fs::metadata(&subdir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "auth dir should be 0700, got {:#o}", dir_mode);
    }

    #[cfg(unix)]
    #[test]
    fn save_with_file_lock_tightens_existing_file() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("auth.json");

        // Create with world-readable permissions
        std::fs::write(&path, "{}").ok();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let creds = OAuthCredentials {
            access: "tok".into(),
            refresh: "ref".into(),
            expires: i64::MAX,
        };

        save_with_file_lock(&path, &creds).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "should tighten loose permissions, got {:#o}", mode);
    }
}
