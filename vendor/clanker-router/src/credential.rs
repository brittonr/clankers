//! Credential manager with automatic OAuth token refresh
//!
//! Handles proactive and reactive token refresh with file locking to prevent
//! race conditions when multiple instances run concurrently.

use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::Mutex;
use tracing::info;
use tracing::warn;

use crate::auth::AuthStore;
use crate::auth::AuthStorePaths;
use crate::auth::StoredCredential;
use crate::error::Result;

/// Callback type for refreshing OAuth tokens.
type RefreshFn = Box<dyn Fn(&str) -> Pin<Box<dyn Future<Output = Result<OAuthTokens>> + Send>> + Send + Sync>;

/// Per-provider credential manager with auto-refresh.
///
/// Thread-safe — uses an internal `Mutex` for interior mutability.
/// Supports both reactive refresh (on 401) and proactive refresh
/// (background timer fires before expiry).
pub struct CredentialManager {
    /// Provider name (e.g. "anthropic")
    provider: String,
    /// Current credential
    credential: Mutex<StoredCredential>,
    /// Auth store paths (single-file or layered seed/runtime)
    auth_paths: AuthStorePaths,
    /// Optional fallback auth path
    fallback_auth_path: Option<PathBuf>,
    /// Callback for refreshing OAuth tokens
    refresh_fn: Option<RefreshFn>,
    /// Handle to cancel the background refresh task on drop
    refresh_task: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// Fresh OAuth tokens from a refresh operation
pub struct OAuthTokens {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at_ms: i64,
}

impl CredentialManager {
    /// Create a new credential manager.
    pub fn new(
        provider: String,
        credential: StoredCredential,
        auth_paths: AuthStorePaths,
        fallback_auth_path: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            provider,
            credential: Mutex::new(credential),
            auth_paths,
            fallback_auth_path,
            refresh_fn: None,
            refresh_task: Mutex::new(None),
        })
    }

    /// Create a new credential manager with a custom refresh function.
    pub fn with_refresh_fn(
        provider: String,
        credential: StoredCredential,
        auth_paths: AuthStorePaths,
        fallback_auth_path: Option<PathBuf>,
        refresh_fn: impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<OAuthTokens>> + Send>>
        + Send
        + Sync
        + 'static,
    ) -> Arc<Self> {
        Arc::new(Self {
            provider,
            credential: Mutex::new(credential),
            auth_paths,
            fallback_auth_path,
            refresh_fn: Some(Box::new(refresh_fn)),
            refresh_task: Mutex::new(None),
        })
    }

    /// Start a background task that proactively refreshes the OAuth token
    /// 5 minutes before it expires. No-op for API key credentials.
    ///
    /// Call this after creation to enable proactive refresh. The task is
    /// cancelled when the `CredentialManager` is dropped.
    pub async fn start_proactive_refresh(self: &Arc<Self>) {
        let cred = self.credential.lock().await;
        if !cred.is_oauth() {
            return; // API keys don't expire
        }
        drop(cred);

        let mgr = Arc::clone(self);
        let handle = tokio::spawn(async move {
            loop {
                // Check when the credential expires
                let sleep_dur = {
                    let cred = mgr.credential.lock().await;
                    match &*cred {
                        StoredCredential::OAuth { expires_at_ms, .. } => {
                            let now_ms = chrono::Utc::now().timestamp_millis();
                            // Refresh 5 minutes before expiry
                            let refresh_at_ms = expires_at_ms - (5 * 60 * 1000);
                            let delay_ms = (refresh_at_ms - now_ms).max(0) as u64;
                            Duration::from_millis(delay_ms)
                        }
                        StoredCredential::ApiKey { .. } => return, // Not OAuth, stop the task
                    }
                };

                if sleep_dur.is_zero() {
                    // Already needs refresh
                    info!("{}: proactive OAuth refresh (token about to expire)", mgr.provider);
                    if let Err(e) = mgr.get_credential().await {
                        warn!("{}: proactive refresh failed: {}", mgr.provider, e);
                    }
                    // After refresh, loop again to schedule next
                    tokio::time::sleep(Duration::from_secs(60)).await;
                } else {
                    tokio::time::sleep(sleep_dur).await;
                    // Time to refresh
                    info!("{}: proactive OAuth refresh (scheduled)", mgr.provider);
                    if let Err(e) = mgr.get_credential().await {
                        warn!("{}: proactive refresh failed: {}", mgr.provider, e);
                        // Retry after 1 minute
                        tokio::time::sleep(Duration::from_secs(60)).await;
                    }
                }
            }
        });

        *self.refresh_task.lock().await = Some(handle);
    }

    /// Stop the proactive refresh background task.
    pub async fn stop_proactive_refresh(&self) {
        if let Some(handle) = self.refresh_task.lock().await.take() {
            handle.abort();
        }
    }

    /// Get the current credential, refreshing if expired.
    pub async fn get_credential(&self) -> Result<StoredCredential> {
        let cred = self.credential.lock().await;
        if !cred.is_expired() {
            return Ok(cred.clone());
        }
        let refresh_token = match cred.refresh_token() {
            Some(rt) => rt.to_string(),
            None => return Ok(cred.clone()), // API keys don't expire
        };
        drop(cred);

        info!("{}: OAuth token expired, refreshing...", self.provider);
        self.do_refresh(&refresh_token).await
    }

    /// Force a refresh (called reactively on 401 errors).
    pub async fn force_refresh(&self) -> Result<StoredCredential> {
        let cred = self.credential.lock().await;
        let refresh_token = cred
            .refresh_token()
            .ok_or_else(|| crate::Error::Auth {
                message: "Cannot refresh a non-OAuth credential".to_string(),
            })?
            .to_string();
        drop(cred);

        info!("{}: Forcing OAuth token refresh (401)", self.provider);
        self.do_refresh(&refresh_token).await
    }

    /// Perform the refresh with file locking.
    async fn do_refresh(&self, refresh_token: &str) -> Result<StoredCredential> {
        let auth_paths = self.auth_paths.clone();
        let provider = self.provider.clone();
        let refresh_token_owned = refresh_token.to_string();

        // Check if another instance already refreshed (read from disk)
        let store = auth_paths.load_effective().into_store();
        if let Some(disk_cred) = store.active_credential(&provider)
            && !disk_cred.is_expired()
        {
            info!("{}: Token was already refreshed by another instance", provider);
            let mut locked = self.credential.lock().await;
            *locked = disk_cred.clone();
            return Ok(disk_cred.clone());
        }

        // Actually refresh
        let refresh_fn = self.refresh_fn.as_ref().ok_or_else(|| crate::Error::Auth {
            message: format!("{}: No refresh function configured", self.provider),
        })?;

        let tokens = refresh_fn(&refresh_token_owned).await?;

        let new_cred = StoredCredential::OAuth {
            access_token: tokens.access_token,
            refresh_token: tokens.refresh_token,
            expires_at_ms: tokens.expires_at_ms,
            label: None,
        };

        // Save to disk with file locking
        self.save_with_lock(&new_cred).await?;

        // Update in-memory
        let mut locked = self.credential.lock().await;
        *locked = new_cred.clone();

        Ok(new_cred)
    }

    /// Save refreshed credential to disk with file locking
    async fn save_with_lock(&self, credential: &StoredCredential) -> Result<()> {
        let auth_paths = self.auth_paths.clone();
        let provider = self.provider.clone();
        let cred = credential.clone();

        tokio::task::spawn_blocking(move || save_with_file_lock(&auth_paths, &provider, &cred))
            .await
            .map_err(|e| crate::Error::Auth {
                message: format!("Save task panicked: {}", e),
            })?
    }

    /// Reload credentials from disk (e.g. after `/login`).
    pub async fn reload_from_disk(&self) {
        let store = self.auth_paths.load_effective().into_store();
        if let Some(cred) = store.active_credential(&self.provider)
            && !cred.is_expired()
        {
            info!("{}: Reloaded credentials from disk", self.provider);
            let mut locked = self.credential.lock().await;
            *locked = cred.clone();
            return;
        }

        // Try fallback
        if let Some(ref fallback) = self.fallback_auth_path {
            let store = AuthStore::load(fallback);
            if let Some(cred) = store.active_credential(&self.provider)
                && !cred.is_expired()
            {
                info!("{}: Reloaded credentials from fallback", self.provider);
                let mut locked = self.credential.lock().await;
                *locked = cred.clone();
            }
        }
    }

    /// Directly update the in-memory credential.
    pub async fn set_credential(&self, credential: StoredCredential) {
        let mut locked = self.credential.lock().await;
        *locked = credential;
    }

    /// Get current token string (without refresh check).
    pub async fn token(&self) -> String {
        self.credential.lock().await.token().to_string()
    }

    /// Whether the current credential is OAuth.
    pub async fn is_oauth(&self) -> bool {
        self.credential.lock().await.is_oauth()
    }
}

/// Save credential to disk with exclusive file lock.
fn save_with_file_lock(auth_paths: &AuthStorePaths, provider: &str, credential: &StoredCredential) -> Result<()> {
    use std::fs;
    use std::io::Write;

    let auth_path = auth_paths.write_path().ok_or_else(|| crate::Error::Auth {
        message: "no auth store write path configured".to_string(),
    })?;

    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(parent, fs::Permissions::from_mode(0o700));
        }
    }
    if !auth_path.exists() {
        let mut f = fs::File::create(auth_path)?;
        f.write_all(b"{}")?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(auth_path, fs::Permissions::from_mode(0o600));
        }
    }

    // Acquire exclusive lock
    let lock_file = fs::File::open(auth_path)?;
    let mut locked = false;
    for _ in 0..30 {
        match fs4::fs_std::FileExt::try_lock_exclusive(&lock_file) {
            Ok(true) => {
                locked = true;
                break;
            }
            _ => std::thread::sleep(Duration::from_secs(1)),
        }
    }

    if !locked {
        warn!("Could not acquire auth file lock after 30s, proceeding without lock");
    }

    // RAII unlock guard
    let _guard = LockGuard {
        locked,
        file: &lock_file,
    };

    // Read, update, write
    let effective = auth_paths.load_effective().into_store();
    let active = effective
        .providers
        .get(provider)
        .and_then(|p| p.active_account.clone())
        .unwrap_or_else(|| "default".to_string());
    let mut store = auth_paths.load_write_store();
    store.set_credential(provider, &active, credential.clone());
    store.save(auth_path)?;

    Ok(())
}

struct LockGuard<'a> {
    locked: bool,
    file: &'a std::fs::File,
}

impl Drop for LockGuard<'_> {
    fn drop(&mut self) {
        if self.locked {
            let _ = fs4::fs_std::FileExt::unlock(self.file);
        }
    }
}
