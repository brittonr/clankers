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
use clanker_router::oauth;
use clanker_router::oauth::OAuthCredentials;
use tokio::sync::Mutex;
use tracing::debug;
use tracing::info;
use tracing::warn;

use super::auth::AuthStoreExt;
use super::auth::Credential;
use crate::error::Result;

/// Manages credentials with automatic refresh for OAuth tokens.
///
/// Thread-safe — uses an internal `Mutex` so it can be shared across
/// the provider and any background tasks.
pub struct CredentialManager {
    /// Current credential (behind a lock for interior mutability)
    credential: Mutex<Credential>,
    /// Path to auth.json for reading/writing refreshed tokens
    auth_path: PathBuf,
    /// Optional fallback auth path (e.g. ~/.pi/agent/auth.json)
    fallback_auth_path: Option<PathBuf>,
}

impl CredentialManager {
    /// Create a new credential manager.
    ///
    /// For OAuth credentials, automatically starts a background task that
    /// proactively refreshes the token before it expires. The task uses a
    /// `Weak` reference, so dropping all strong `Arc` refs stops it.
    pub fn new(credential: Credential, auth_path: PathBuf, fallback_auth_path: Option<PathBuf>) -> Arc<Self> {
        let is_oauth = credential.is_oauth();
        let mgr = Arc::new(Self {
            credential: Mutex::new(credential),
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
        // Drop the lock before doing I/O
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

    /// Perform the actual refresh.
    ///
    /// 1. Check disk (lockless) — another instance may have already refreshed
    /// 2. HTTP refresh (async, no file lock held)
    /// 3. Save to disk (spawn_blocking, brief exclusive file lock)
    /// 4. Update in-memory credential
    async fn do_refresh(&self, refresh_token: &str) -> Result<Credential> {
        // 1. Quick disk check — skip HTTP if another instance already refreshed
        let auth_path = self.auth_path.clone();
        let store = {
            let path = auth_path.clone();
            tokio::task::spawn_blocking(move || AuthStore::load(&path))
                .await
                .map_err(|e| crate::error::auth_err(format!("Disk read panicked: {e}")))?
        };
        if let Some(cred) = store.active_credential("anthropic")
            && !cred.is_expired()
        {
            info!("Token already refreshed by another instance");
            let fresh = cred.clone();
            *self.credential.lock().await = fresh.clone();
            return Ok(fresh);
        }

        // 2. HTTP refresh (no lock held — this can take hundreds of ms)
        let new_creds = oauth::refresh_token(refresh_token).await?;

        // 3. Save to disk with file locking
        let creds_clone = new_creds.clone();
        let path = self.auth_path.clone();
        tokio::task::spawn_blocking(move || save_with_file_lock(&path, &creds_clone))
            .await
            .map_err(|e| crate::error::auth_err(format!("Save task panicked: {e}")))??;

        info!("OAuth token refreshed, new expiry: {}", new_creds.expires);

        // 4. Update in-memory
        let new_credential = new_creds.to_stored();
        *self.credential.lock().await = new_credential.clone();

        Ok(new_credential)
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
        if let Some(creds) = store.active_credentials()
            && !creds.is_expired()
        {
            info!("Reloaded credentials from disk after login");
            *self.credential.lock().await = creds.to_stored();
            return;
        }

        // Try fallback path (e.g. ~/.pi/agent/auth.json)
        if let Some(ref fallback) = self.fallback_auth_path {
            let store = AuthStore::load(fallback);
            if let Some(creds) = store.active_credentials()
                && !creds.is_expired()
            {
                info!("Reloaded credentials from fallback auth path");
                *self.credential.lock().await = creds.to_stored();
            }
        }
    }

    /// Directly update the in-memory credential (e.g. after a fresh login).
    pub async fn set_credential(&self, credential: Credential) {
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

/// Save refreshed credentials to disk under an exclusive file lock.
///
/// Runs in `spawn_blocking` because `fs4` lock operations are blocking.
/// The lock is held only for the read-modify-write cycle (~ms), not during
/// any network calls.
fn save_with_file_lock(auth_path: &std::path::Path, creds: &OAuthCredentials) -> Result<()> {
    use std::fs;
    use std::io::Write;

    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if !auth_path.exists() {
        let mut f =
            fs::File::create(auth_path).map_err(|e| crate::error::auth_err(format!("Create auth file: {e}")))?;
        f.write_all(b"{}").ok();
    }

    let lock_file =
        fs::File::open(auth_path).map_err(|e| crate::error::auth_err(format!("Open auth file for locking: {e}")))?;

    let mut locked = false;
    for attempt in 0..30 {
        match fs4::fs_std::FileExt::try_lock_exclusive(&lock_file) {
            Ok(true) => {
                locked = true;
                break;
            }
            Ok(false) | Err(_) => {
                if attempt < 29 {
                    std::thread::sleep(Duration::from_secs(1));
                }
            }
        }
    }

    if !locked {
        warn!("Could not acquire auth file lock after 30s, proceeding without lock");
    }

    let _guard = UnlockGuard {
        locked,
        file: &lock_file,
    };

    // Read-modify-write under lock
    let mut store = AuthStore::load(auth_path);
    let account_name = store.active_account_name().to_string();
    store.set_credentials(&account_name, creds.clone());
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
            let _ = fs4::fs_std::FileExt::unlock(self.file);
        }
    }
}
