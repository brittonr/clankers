//! Credential manager with automatic OAuth token refresh.
//!
//! Handles proactive and reactive token refresh with file locking to prevent
//! race conditions when multiple clankers instances run concurrently.
//!
//! Follows the same pattern as pi's `AuthStorage.refreshOAuthTokenWithLock()`:
//! 1. Acquire exclusive file lock on auth.json
//! 2. Re-read the file (another instance may have already refreshed)
//! 3. If still expired, refresh and save
//! 4. Release lock

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use clankers_router::auth::AuthStore;
use clankers_router::oauth;
use clankers_router::oauth::OAuthCredentials;
use tokio::sync::Mutex;
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
    pub fn new(credential: Credential, auth_path: PathBuf, fallback_auth_path: Option<PathBuf>) -> Arc<Self> {
        Arc::new(Self {
            credential: Mutex::new(credential),
            auth_path,
            fallback_auth_path,
        })
    }

    /// Get the current credential, refreshing if expired.
    ///
    /// For API keys, returns immediately. For OAuth tokens, checks expiry
    /// and refreshes proactively if needed.
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
    /// Re-checks after acquiring the lock in case another call already refreshed.
    pub async fn force_refresh(&self) -> Result<Credential> {
        let cred = self.credential.lock().await;
        let refresh_token = match cred.refresh_token() {
            Some(rt) => rt.to_string(),
            None => {
                return Err(crate::error::Error::ProviderAuth {
                    message: "Cannot refresh a non-OAuth credential".to_string(),
                });
            }
        };
        drop(cred);

        info!("Forcing OAuth token refresh (401 received)");
        self.do_refresh(&refresh_token).await
    }

    /// Perform the actual refresh with file locking.
    async fn do_refresh(&self, refresh_token: &str) -> Result<Credential> {
        let auth_path = self.auth_path.clone();
        let fallback_path = self.fallback_auth_path.clone();
        let refresh_token_owned = refresh_token.to_string();

        let new_creds = tokio::task::spawn_blocking(move || {
            refresh_with_file_lock(&auth_path, fallback_path.as_deref(), &refresh_token_owned)
        })
        .await
        .map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Refresh task panicked: {}", e),
        })??;

        let creds = match new_creds {
            RefreshResult::Refreshed(creds) | RefreshResult::AlreadyValid(creds) => creds,
        };

        // Update our in-memory credential
        let new_credential = creds.to_stored();
        let mut locked = self.credential.lock().await;
        *locked = new_credential.clone();

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
            let mut locked = self.credential.lock().await;
            *locked = creds.to_stored();
        }
    }

    /// Directly update the in-memory credential (e.g. after a fresh login).
    pub async fn set_credential(&self, credential: Credential) {
        let mut locked = self.credential.lock().await;
        *locked = credential;
    }

    /// Get the current token string (without refresh check).
    pub async fn token(&self) -> String {
        self.credential.lock().await.token().to_string()
    }
}

enum RefreshResult {
    Refreshed(OAuthCredentials),
    AlreadyValid(OAuthCredentials),
}

/// Refresh OAuth token with file locking (runs in spawn_blocking).
fn refresh_with_file_lock(
    auth_path: &std::path::Path,
    _fallback_path: Option<&std::path::Path>,
    refresh_token: &str,
) -> Result<RefreshResult> {
    use std::fs;
    use std::io::Write;

    if let Some(parent) = auth_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    if !auth_path.exists() {
        let mut f = fs::File::create(auth_path).map_err(|e| crate::error::Error::ProviderAuth {
            message: format!("Failed to create auth file: {}", e),
        })?;
        f.write_all(b"{}").ok();
    }

    let lock_file = fs::File::open(auth_path).map_err(|e| crate::error::Error::ProviderAuth {
        message: format!("Failed to open auth file for locking: {}", e),
    })?;

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

    let _unlock_guard = scopeguard(locked, &lock_file);

    // Re-read — another instance may have already refreshed
    let store = AuthStore::load(auth_path);
    if let Some(cred) = store.active_credential("anthropic")
        && !cred.is_expired()
        && let Some(creds) = OAuthCredentials::from_stored(cred)
    {
        info!("Token was already refreshed by another instance");
        return Ok(RefreshResult::AlreadyValid(creds));
    }

    // Still expired — refresh
    let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| {
        crate::error::Error::ProviderAuth {
            message: format!("Failed to create runtime for token refresh: {}", e),
        }
    })?;

    let new_creds = rt.block_on(oauth::refresh_token(refresh_token))?;

    // Save to disk
    let mut store = AuthStore::load(auth_path);
    let account_name = store.active_account_name().to_string();
    store.set_credentials(&account_name, new_creds.clone());
    store.save(auth_path).map_err(|e| crate::error::Error::ProviderAuth {
        message: format!("Failed to save refreshed credentials: {}", e),
    })?;

    info!("OAuth token refreshed successfully, new expiry: {}", new_creds.expires);

    Ok(RefreshResult::Refreshed(new_creds))
}

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

fn scopeguard<'a>(locked: bool, file: &'a std::fs::File) -> UnlockGuard<'a> {
    UnlockGuard { locked, file }
}
