//! Auth — delegates to `clankers_router::auth` and `clankers_router::oauth`
//!
//! All credential storage uses clankers-router's multi-provider auth store
//! at `~/.config/clankers-router/auth.json`.

use std::path::Path;

// Re-export the canonical types from clankers-router
pub use clankers_router::auth::{
    AccountInfo, AuthStore, ProviderAuth, StoredCredential, env_var_for_provider, is_oauth_token,
};
pub use clankers_router::oauth::OAuthCredentials;

/// Resolved credential for making API calls.
///
/// Thin wrapper around [`StoredCredential`] that preserves the clankers-internal
/// API (`token()`, `is_oauth()`, `needs_refresh()`).
pub type Credential = StoredCredential;

/// Extension methods on `StoredCredential` used by clankers internals.
pub trait CredentialExt {
    fn needs_refresh(&self) -> bool;
}

impl CredentialExt for StoredCredential {
    fn needs_refresh(&self) -> bool {
        self.is_expired()
    }
}

/// Anthropic-specific convenience methods on [`AuthStore`].
///
/// All methods delegate to the multi-provider store with `provider = "anthropic"`.
/// Lives here (rather than in `provider::anthropic`) because it is used by
/// commands, slash-commands, and mode setup — not just the Anthropic backend.
pub trait AuthStoreExt {
    fn active_account_name(&self) -> &str;
    fn active_credentials(&self) -> Option<OAuthCredentials>;
    fn set_credentials(&mut self, account: &str, creds: OAuthCredentials);
    fn switch_anthropic_account(&mut self, account: &str) -> bool;
    fn remove_anthropic_account(&mut self, account: &str) -> bool;
    fn list_anthropic_accounts(&self) -> Vec<AccountInfo>;
    fn account_summary(&self) -> String;
    fn save_clankers(&self, path: &Path) -> crate::error::Result<()>;
}

impl AuthStoreExt for AuthStore {
    fn active_account_name(&self) -> &str {
        self.providers.get("anthropic").and_then(|p| p.active_account.as_deref()).unwrap_or("default")
    }

    fn active_credentials(&self) -> Option<OAuthCredentials> {
        let cred = self.active_credential("anthropic")?;
        OAuthCredentials::from_stored(cred)
    }

    fn set_credentials(&mut self, account: &str, creds: OAuthCredentials) {
        self.set_credential("anthropic", account, creds.to_stored());
    }

    fn switch_anthropic_account(&mut self, account: &str) -> bool {
        clankers_router::auth::AuthStore::switch_account(self, "anthropic", account)
    }

    fn remove_anthropic_account(&mut self, account: &str) -> bool {
        clankers_router::auth::AuthStore::remove_account(self, "anthropic", account)
    }

    fn list_anthropic_accounts(&self) -> Vec<AccountInfo> {
        self.list_accounts("anthropic")
    }

    fn account_summary(&self) -> String {
        let accounts = self.list_anthropic_accounts();
        if accounts.is_empty() {
            return "No accounts configured. Use /login or `clankers auth login` to add one.".to_string();
        }
        let mut out = String::new();
        for info in &accounts {
            let marker = if info.is_active { "▸" } else { " " };
            let status = if info.is_expired { " (expired)" } else { "" };
            let label = info.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
            out.push_str(&format!("{} {}{}{}\n", marker, info.name, label, status));
        }
        out
    }

    fn save_clankers(&self, path: &Path) -> crate::error::Result<()> {
        self.save(path).map_err(|e| e.into())
    }
}

/// Resolve credentials for API access (Anthropic).
///
/// Delegates to `clankers_router::auth::resolve_credential`.
pub fn resolve_credential(runtime_override: Option<&str>, auth_store_path: &Path) -> Option<Credential> {
    resolve_credential_with_fallback(runtime_override, auth_store_path, None, None)
}

/// Resolve credentials with fallback and account selection.
pub fn resolve_credential_with_fallback(
    runtime_override: Option<&str>,
    auth_store_path: &Path,
    fallback_auth_path: Option<&Path>,
    account: Option<&str>,
) -> Option<Credential> {
    let store = AuthStore::load(auth_store_path);
    let fallback = fallback_auth_path.map(AuthStore::load);

    // If a specific account was requested, try that first
    if let Some(acct) = account
        && let Some(cred) = store.credential_for("anthropic", acct)
    {
        return Some(cred.clone());
    }

    clankers_router::auth::resolve_credential("anthropic", runtime_override, &store, fallback.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_store_ext_active_account() {
        let store = AuthStore::default();
        assert_eq!(store.active_account_name(), "default");
    }

    #[test]
    fn test_auth_store_ext_set_and_get() {
        let mut store = AuthStore::default();
        let creds = OAuthCredentials {
            access: "tok".into(),
            refresh: "ref".into(),
            expires: i64::MAX,
        };
        store.set_credentials("work", creds);
        // First set auto-activates
        assert!(store.active_credentials().is_some());
    }

    #[test]
    fn test_auth_store_ext_switch() {
        let mut store = AuthStore::default();
        let c = OAuthCredentials {
            access: "a".into(),
            refresh: "r".into(),
            expires: i64::MAX,
        };
        store.set_credentials("one", c.clone());
        store.set_credentials("two", OAuthCredentials {
            access: "b".into(),
            ..c
        });
        assert!(store.switch_anthropic_account("two"));
        assert_eq!(store.active_credentials().expect("active credentials should exist").access, "b");
    }

    #[test]
    fn test_auth_store_ext_remove() {
        let mut store = AuthStore::default();
        let c = OAuthCredentials {
            access: "a".into(),
            refresh: "r".into(),
            expires: i64::MAX,
        };
        store.set_credentials("x", c);
        assert!(store.remove_anthropic_account("x"));
        assert!(store.active_credentials().is_none());
    }

    #[test]
    fn test_credential_needs_refresh() {
        let api_key = StoredCredential::ApiKey {
            api_key: "sk-test".into(),
            label: None,
        };
        assert!(!api_key.needs_refresh());

        let expired_oauth = StoredCredential::OAuth {
            access_token: "t".into(),
            refresh_token: "r".into(),
            expires_at_ms: 0,
            label: None,
        };
        assert!(expired_oauth.needs_refresh());
    }
}
