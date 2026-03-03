//! Multi-provider auth store with OAuth + API key support
//!
//! Stores credentials for multiple providers and accounts in a single JSON file.
//! Supports:
//! - API key credentials (static tokens from env or config)
//! - OAuth credentials with automatic refresh
//! - Multiple named accounts per provider
//! - Round-robin credential selection
//!
//! ## Auth store format (`auth.json`)
//!
//! ```json
//! {
//!   "version": 2,
//!   "providers": {
//!     "anthropic": {
//!       "active_account": "default",
//!       "accounts": {
//!         "default": {
//!           "credential_type": "oauth",
//!           "access_token": "sk-ant-oat-...",
//!           "refresh_token": "sk-ant-ort-...",
//!           "expires_at_ms": 1700000000000
//!         },
//!         "work": {
//!           "credential_type": "api_key",
//!           "api_key": "sk-ant-api-..."
//!         }
//!       }
//!     },
//!     "openai": {
//!       "active_account": "default",
//!       "accounts": {
//!         "default": {
//!           "credential_type": "api_key",
//!           "api_key": "sk-..."
//!         }
//!       }
//!     }
//!   }
//! }
//! ```

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

/// Top-level auth store (serialized to auth.json)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthStore {
    /// Schema version (current: 2)
    #[serde(default = "default_version")]
    pub version: u32,

    /// Per-provider credential storage
    #[serde(default)]
    pub providers: HashMap<String, ProviderAuth>,

    // ── Legacy compatibility (v1 format from pi) ─────────────────────────
    /// Legacy Anthropic OAuth credentials (v1 format)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anthropic: Option<LegacyOAuthCredentials>,
}

fn default_version() -> u32 {
    2
}

/// Per-provider auth with multiple named accounts
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderAuth {
    /// Which account is currently active
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_account: Option<String>,

    /// Named accounts
    #[serde(default)]
    pub accounts: HashMap<String, StoredCredential>,
}

/// A stored credential (API key or OAuth)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "credential_type")]
pub enum StoredCredential {
    /// Static API key
    #[serde(rename = "api_key")]
    ApiKey {
        api_key: String,
        /// Optional display label
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
    /// OAuth credentials with refresh capability
    #[serde(rename = "oauth")]
    OAuth {
        access_token: String,
        refresh_token: String,
        /// Expiration timestamp in milliseconds since epoch
        expires_at_ms: i64,
        /// Optional display label
        #[serde(skip_serializing_if = "Option::is_none")]
        label: Option<String>,
    },
}

impl StoredCredential {
    /// Get the token string for use in requests
    pub fn token(&self) -> &str {
        match self {
            Self::ApiKey { api_key, .. } => api_key,
            Self::OAuth { access_token, .. } => access_token,
        }
    }

    /// Whether this is an OAuth credential
    pub fn is_oauth(&self) -> bool {
        matches!(self, Self::OAuth { .. })
    }

    /// Whether the credential is expired (only applies to OAuth)
    pub fn is_expired(&self) -> bool {
        match self {
            Self::ApiKey { .. } => false,
            Self::OAuth { expires_at_ms, .. } => chrono::Utc::now().timestamp_millis() >= *expires_at_ms,
        }
    }

    /// Get the refresh token (only for OAuth)
    pub fn refresh_token(&self) -> Option<&str> {
        match self {
            Self::OAuth { refresh_token, .. } => Some(refresh_token),
            _ => None,
        }
    }

    /// Get the optional label
    pub fn label(&self) -> Option<&str> {
        match self {
            Self::ApiKey { label, .. } => label.as_deref(),
            Self::OAuth { label, .. } => label.as_deref(),
        }
    }
}

/// Legacy OAuth credentials (v1 format, compatible with pi's auth.json)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyOAuthCredentials {
    pub access: String,
    pub refresh: String,
    pub expires: i64,
}

impl LegacyOAuthCredentials {
    pub fn is_expired(&self) -> bool {
        chrono::Utc::now().timestamp_millis() >= self.expires
    }
}

impl AuthStore {
    /// Load auth store from a file path
    pub fn load(path: &Path) -> Self {
        let mut store: Self =
            std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();

        store.migrate_legacy();
        store
    }

    /// Save auth store to a file path
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Migrate legacy v1 `anthropic` field into the v2 providers map
    fn migrate_legacy(&mut self) {
        if let Some(ref legacy) = self.anthropic {
            let provider = self.providers.entry("anthropic".to_string()).or_default();

            if !provider.accounts.contains_key("default") {
                provider.accounts.insert("default".to_string(), StoredCredential::OAuth {
                    access_token: legacy.access.clone(),
                    refresh_token: legacy.refresh.clone(),
                    expires_at_ms: legacy.expires,
                    label: None,
                });
            }
            if provider.active_account.is_none() {
                provider.active_account = Some("default".to_string());
            }
        }
    }

    // ── Provider-level operations ────────────────────────────────────────

    /// Get credentials for a provider's active account
    pub fn active_credential(&self, provider: &str) -> Option<&StoredCredential> {
        let prov = self.providers.get(provider)?;
        let account = prov.active_account.as_deref().unwrap_or("default");
        prov.accounts.get(account)
    }

    /// Get credentials for a specific provider + account
    pub fn credential_for(&self, provider: &str, account: &str) -> Option<&StoredCredential> {
        self.providers.get(provider)?.accounts.get(account)
    }

    /// Set credentials for a provider + account
    pub fn set_credential(&mut self, provider: &str, account: &str, credential: StoredCredential) {
        let prov = self.providers.entry(provider.to_string()).or_default();
        prov.accounts.insert(account.to_string(), credential);

        // Auto-set as active if no active account
        if prov.active_account.is_none() {
            prov.active_account = Some(account.to_string());
        }

        // Keep legacy field in sync for Anthropic
        if provider == "anthropic"
            && let Some(cred) = prov.accounts.get(account)
            && let StoredCredential::OAuth {
                access_token,
                refresh_token,
                expires_at_ms,
                ..
            } = cred
        {
            self.anthropic = Some(LegacyOAuthCredentials {
                access: access_token.clone(),
                refresh: refresh_token.clone(),
                expires: *expires_at_ms,
            });
        }
    }

    /// Switch the active account for a provider
    pub fn switch_account(&mut self, provider: &str, account: &str) -> bool {
        if let Some(prov) = self.providers.get_mut(provider)
            && prov.accounts.contains_key(account)
        {
            prov.active_account = Some(account.to_string());
            return true;
        }
        false
    }

    /// Remove an account from a provider
    pub fn remove_account(&mut self, provider: &str, account: &str) -> bool {
        if let Some(prov) = self.providers.get_mut(provider) {
            let removed = prov.accounts.remove(account).is_some();
            if removed && prov.active_account.as_deref() == Some(account) {
                prov.active_account = prov.accounts.keys().next().cloned();
            }
            return removed;
        }
        false
    }

    /// List all providers that have credentials
    pub fn configured_providers(&self) -> Vec<&str> {
        self.providers.iter().filter(|(_, v)| !v.accounts.is_empty()).map(|(k, _)| k.as_str()).collect()
    }

    /// List all accounts for a provider
    pub fn list_accounts(&self, provider: &str) -> Vec<AccountInfo> {
        let Some(prov) = self.providers.get(provider) else {
            return Vec::new();
        };
        let active = prov.active_account.as_deref().unwrap_or("default");
        prov.accounts
            .iter()
            .map(|(name, cred)| AccountInfo {
                provider: provider.to_string(),
                name: name.clone(),
                label: cred.label().map(String::from),
                is_active: name == active,
                is_expired: cred.is_expired(),
                is_oauth: cred.is_oauth(),
            })
            .collect()
    }

    /// Get all non-expired credentials for a provider (for credential pools).
    ///
    /// Returns `(account_name, credential)` pairs ordered with the active
    /// account first.
    pub fn all_credentials(&self, provider: &str) -> Vec<(String, StoredCredential)> {
        let Some(prov) = self.providers.get(provider) else {
            return Vec::new();
        };

        let active = prov.active_account.as_deref().unwrap_or("default");
        let mut creds: Vec<(String, StoredCredential)> = prov
            .accounts
            .iter()
            .filter(|(_, cred)| !cred.is_expired())
            .map(|(name, cred)| (name.clone(), cred.clone()))
            .collect();

        // Sort: active account first, then alphabetical
        creds.sort_by(|(a, _), (b, _)| {
            let a_active = a == active;
            let b_active = b == active;
            b_active.cmp(&a_active).then_with(|| a.cmp(b))
        });

        creds
    }

    /// Summary string for all configured credentials
    pub fn summary(&self) -> String {
        let providers = self.configured_providers();
        if providers.is_empty() {
            return "No credentials configured.".to_string();
        }

        let mut out = String::new();
        for provider in &providers {
            out.push_str(&format!("{}:\n", provider));
            for info in self.list_accounts(provider) {
                let marker = if info.is_active { "▸" } else { " " };
                let kind = if info.is_oauth { "oauth" } else { "api-key" };
                let status = if info.is_expired { " (expired)" } else { "" };
                let label = info.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
                out.push_str(&format!("  {} {} [{}]{}{}\n", marker, info.name, kind, label, status));
            }
        }
        out
    }
}

/// Info about an account for display
#[derive(Debug, Clone)]
pub struct AccountInfo {
    pub provider: String,
    pub name: String,
    pub label: Option<String>,
    pub is_active: bool,
    pub is_expired: bool,
    pub is_oauth: bool,
}

// ── Env var resolution ──────────────────────────────────────────────────

/// Well-known environment variables for provider API keys
pub fn env_var_for_provider(provider: &str) -> Option<&'static str> {
    match provider {
        "anthropic" => Some("ANTHROPIC_API_KEY"),
        "openai" => Some("OPENAI_API_KEY"),
        "openrouter" => Some("OPENROUTER_API_KEY"),
        "google" | "gemini" => Some("GOOGLE_API_KEY"),
        "huggingface" | "hf" => Some("HF_TOKEN"),
        "mistral" => Some("MISTRAL_API_KEY"),
        "groq" => Some("GROQ_API_KEY"),
        "deepseek" => Some("DEEPSEEK_API_KEY"),
        "together" => Some("TOGETHER_API_KEY"),
        "fireworks" => Some("FIREWORKS_API_KEY"),
        "perplexity" => Some("PERPLEXITY_API_KEY"),
        "cohere" => Some("COHERE_API_KEY"),
        "xai" | "grok" => Some("XAI_API_KEY"),
        _ => None,
    }
}

/// Resolve a credential for a provider using the standard priority chain:
///
/// 1. Runtime override (CLI `--api-key` flag)
/// 2. Environment variable
/// 3. Auth store (active account)
/// 4. Fallback auth store path
pub fn resolve_credential(
    provider: &str,
    runtime_override: Option<&str>,
    auth_store: &AuthStore,
    fallback_store: Option<&AuthStore>,
) -> Option<StoredCredential> {
    // 1. Runtime override
    if let Some(key) = runtime_override
        && !key.is_empty()
    {
        return Some(if is_oauth_token(key) {
            StoredCredential::OAuth {
                access_token: key.to_string(),
                refresh_token: String::new(),
                expires_at_ms: i64::MAX,
                label: Some("cli-override".to_string()),
            }
        } else {
            StoredCredential::ApiKey {
                api_key: key.to_string(),
                label: Some("cli-override".to_string()),
            }
        });
    }

    // 2. Environment variable
    if let Some(env_var) = env_var_for_provider(provider)
        && let Ok(key) = std::env::var(env_var)
        && !key.is_empty()
    {
        return Some(if is_oauth_token(&key) {
            StoredCredential::OAuth {
                access_token: key,
                refresh_token: String::new(),
                expires_at_ms: i64::MAX,
                label: Some(format!("env:{}", env_var)),
            }
        } else {
            StoredCredential::ApiKey {
                api_key: key,
                label: Some(format!("env:{}", env_var)),
            }
        });
    }

    // 3. Auth store
    if let Some(cred) = auth_store.active_credential(provider) {
        return Some(cred.clone());
    }

    // 4. Fallback store
    if let Some(fallback) = fallback_store
        && let Some(cred) = fallback.active_credential(provider)
    {
        return Some(cred.clone());
    }

    None
}

/// Check if a token looks like an Anthropic OAuth token
pub fn is_oauth_token(token: &str) -> bool {
    token.contains("sk-ant-oat")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_store() {
        let store = AuthStore::default();
        assert!(store.active_credential("anthropic").is_none());
        assert!(store.configured_providers().is_empty());
    }

    #[test]
    fn test_set_and_get_api_key() {
        let mut store = AuthStore::default();
        store.set_credential("openai", "default", StoredCredential::ApiKey {
            api_key: "sk-test".into(),
            label: None,
        });
        let cred = store.active_credential("openai").unwrap();
        assert_eq!(cred.token(), "sk-test");
        assert!(!cred.is_oauth());
        assert!(!cred.is_expired());
    }

    #[test]
    fn test_set_and_get_oauth() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "default", StoredCredential::OAuth {
            access_token: "oat-test".into(),
            refresh_token: "ort-test".into(),
            expires_at_ms: i64::MAX,
            label: None,
        });
        let cred = store.active_credential("anthropic").unwrap();
        assert_eq!(cred.token(), "oat-test");
        assert!(cred.is_oauth());
        assert!(!cred.is_expired());
    }

    #[test]
    fn test_multi_provider() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "default", StoredCredential::ApiKey {
            api_key: "ant-key".into(),
            label: None,
        });
        store.set_credential("openai", "default", StoredCredential::ApiKey {
            api_key: "oai-key".into(),
            label: None,
        });

        let mut providers = store.configured_providers();
        providers.sort();
        assert_eq!(providers, vec!["anthropic", "openai"]);
    }

    #[test]
    fn test_switch_account() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "personal", StoredCredential::ApiKey {
            api_key: "key-a".into(),
            label: None,
        });
        store.set_credential("anthropic", "work", StoredCredential::ApiKey {
            api_key: "key-b".into(),
            label: None,
        });
        // First set is auto-activated
        assert_eq!(store.active_credential("anthropic").unwrap().token(), "key-a");

        assert!(store.switch_account("anthropic", "work"));
        assert_eq!(store.active_credential("anthropic").unwrap().token(), "key-b");

        assert!(!store.switch_account("anthropic", "nonexistent"));
    }

    #[test]
    fn test_remove_account() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "a", StoredCredential::ApiKey {
            api_key: "k1".into(),
            label: None,
        });
        store.set_credential("anthropic", "b", StoredCredential::ApiKey {
            api_key: "k2".into(),
            label: None,
        });

        assert!(store.remove_account("anthropic", "a"));
        assert!(store.active_credential("anthropic").is_some());
    }

    #[test]
    fn test_legacy_migration() {
        let mut store = AuthStore::default();
        store.anthropic = Some(LegacyOAuthCredentials {
            access: "old-token".into(),
            refresh: "old-refresh".into(),
            expires: i64::MAX,
        });
        store.migrate_legacy();

        let cred = store.active_credential("anthropic").unwrap();
        assert_eq!(cred.token(), "old-token");
        assert!(cred.is_oauth());
    }

    #[test]
    fn test_resolve_credential_priority() {
        let mut store = AuthStore::default();
        store.set_credential("openai", "default", StoredCredential::ApiKey {
            api_key: "from-store".into(),
            label: None,
        });

        // Runtime override wins
        let cred = resolve_credential("openai", Some("override-key"), &store, None);
        assert_eq!(cred.unwrap().token(), "override-key");

        // No override: falls back to store
        let cred = resolve_credential("openai", None, &store, None);
        assert_eq!(cred.unwrap().token(), "from-store");
    }

    #[test]
    fn test_env_var_mapping() {
        assert_eq!(env_var_for_provider("anthropic"), Some("ANTHROPIC_API_KEY"));
        assert_eq!(env_var_for_provider("openai"), Some("OPENAI_API_KEY"));
        assert_eq!(env_var_for_provider("unknown"), None);
    }

    #[test]
    fn test_summary() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "work", StoredCredential::ApiKey {
            api_key: "k".into(),
            label: Some("Work account".into()),
        });
        let s = store.summary();
        assert!(s.contains("anthropic"));
        assert!(s.contains("work"));
        assert!(s.contains("Work account"));
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");

        let mut store = AuthStore::default();
        store.set_credential("anthropic", "default", StoredCredential::ApiKey {
            api_key: "test-key".into(),
            label: None,
        });
        store.save(&path).unwrap();

        let loaded = AuthStore::load(&path);
        assert_eq!(loaded.active_credential("anthropic").unwrap().token(), "test-key");
    }

    #[test]
    fn test_all_credentials_empty() {
        let store = AuthStore::default();
        assert!(store.all_credentials("anthropic").is_empty());
    }

    #[test]
    fn test_all_credentials_single() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "default", StoredCredential::ApiKey {
            api_key: "key-1".into(),
            label: None,
        });
        let creds = store.all_credentials("anthropic");
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].0, "default");
        assert_eq!(creds[0].1.token(), "key-1");
    }

    #[test]
    fn test_all_credentials_multi_account_active_first() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "personal", StoredCredential::ApiKey {
            api_key: "key-personal".into(),
            label: None,
        });
        store.set_credential("anthropic", "work", StoredCredential::ApiKey {
            api_key: "key-work".into(),
            label: None,
        });
        store.set_credential("anthropic", "backup", StoredCredential::ApiKey {
            api_key: "key-backup".into(),
            label: None,
        });

        // "personal" is active (first set is auto-activated)
        let creds = store.all_credentials("anthropic");
        assert_eq!(creds.len(), 3);
        assert_eq!(creds[0].0, "personal"); // active first

        // Switch active to "work"
        store.switch_account("anthropic", "work");
        let creds = store.all_credentials("anthropic");
        assert_eq!(creds[0].0, "work"); // now work is first
    }

    #[test]
    fn test_all_credentials_filters_expired() {
        let mut store = AuthStore::default();
        store.set_credential("anthropic", "valid", StoredCredential::ApiKey {
            api_key: "key-1".into(),
            label: None,
        });
        store.set_credential("anthropic", "expired", StoredCredential::OAuth {
            access_token: "expired-token".into(),
            refresh_token: "rt".into(),
            expires_at_ms: 0, // expired
            label: None,
        });

        let creds = store.all_credentials("anthropic");
        assert_eq!(creds.len(), 1);
        assert_eq!(creds[0].0, "valid");
    }
}
