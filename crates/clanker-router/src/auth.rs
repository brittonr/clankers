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
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

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
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
            Self::ApiKey { .. } => None,
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

    /// Save auth store to a file path.
    ///
    /// Sets restrictive file permissions (0600) since the file contains
    /// plaintext API keys and OAuth tokens.
    pub fn save(&self, path: &Path) -> crate::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700));
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, &json)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
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
            writeln!(out, "{}", provider).unwrap();
            for info in self.list_accounts(provider) {
                let marker = if info.is_active { "▸" } else { " " };
                let kind = if info.is_oauth { "oauth" } else { "api-key" };
                let status = if info.is_expired { " (expired)" } else { "" };
                let label = info.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
                writeln!(out, "  {} {} [{}]{}{}", marker, info.name, kind, label, status).unwrap();
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthRecordSource {
    File,
    Seed,
    Runtime,
}

impl AuthRecordSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Seed => "seed",
            Self::Runtime => "runtime",
        }
    }
}

#[derive(Debug, Clone)]
pub struct SourcedAccountInfo {
    pub info: AccountInfo,
    pub source: AuthRecordSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProviderAccountExport {
    #[serde(default = "default_export_version")]
    pub version: u32,
    pub provider: String,
    pub account: String,
    #[serde(default)]
    pub active: bool,
    pub credential: StoredCredential,
}

fn default_export_version() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImportTarget {
    Auto,
    File,
    Seed,
    Runtime,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthStorePaths {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed_file: Option<PathBuf>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub runtime_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct EffectiveAuthStore {
    store: AuthStore,
    sources: HashMap<String, HashMap<String, AuthRecordSource>>,
}

impl EffectiveAuthStore {
    pub fn store(&self) -> &AuthStore {
        &self.store
    }

    pub fn into_store(self) -> AuthStore {
        self.store
    }

    pub fn source_for(&self, provider: &str, account: &str) -> Option<AuthRecordSource> {
        self.sources.get(provider).and_then(|accounts| accounts.get(account)).copied()
    }

    pub fn list_accounts_with_sources(&self, provider: &str) -> Vec<SourcedAccountInfo> {
        self.store
            .list_accounts(provider)
            .into_iter()
            .map(|info| SourcedAccountInfo {
                source: self.source_for(provider, &info.name).unwrap_or(AuthRecordSource::File),
                info,
            })
            .collect()
    }

    pub fn export_account(&self, provider: &str, account: &str) -> Option<ProviderAccountExport> {
        let credential = self.store.credential_for(provider, account)?.clone();
        Some(ProviderAccountExport {
            version: default_export_version(),
            provider: provider.to_string(),
            account: account.to_string(),
            active: self.store.active_credential(provider).is_some()
                && self.store.providers.get(provider).and_then(|p| p.active_account.as_deref()) == Some(account),
            credential,
        })
    }
}

impl AuthStorePaths {
    pub fn single(path: PathBuf) -> Self {
        Self {
            auth_file: Some(path),
            seed_file: None,
            runtime_file: None,
        }
    }

    pub fn layered(seed_file: PathBuf, runtime_file: PathBuf) -> Self {
        Self {
            auth_file: None,
            seed_file: Some(seed_file),
            runtime_file: Some(runtime_file),
        }
    }

    pub fn is_layered(&self) -> bool {
        self.seed_file.is_some() || self.runtime_file.is_some()
    }

    pub fn write_source(&self) -> AuthRecordSource {
        if self.runtime_file.is_some() {
            AuthRecordSource::Runtime
        } else if self.seed_file.is_some() {
            AuthRecordSource::Seed
        } else {
            AuthRecordSource::File
        }
    }

    pub fn write_path(&self) -> Option<&Path> {
        self.runtime_file
            .as_deref()
            .or(self.auth_file.as_deref())
            .or(self.seed_file.as_deref())
    }

    pub fn pending_oauth_base_dir(&self) -> Option<PathBuf> {
        self.write_path()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
    }

    pub fn load_effective(&self) -> EffectiveAuthStore {
        if !self.is_layered() {
            let store = self.auth_file.as_deref().map(AuthStore::load).unwrap_or_default();
            let mut sources = HashMap::new();
            for provider in store.configured_providers() {
                let per_provider = store
                    .list_accounts(provider)
                    .into_iter()
                    .map(|info| (info.name, AuthRecordSource::File))
                    .collect();
                sources.insert(provider.to_string(), per_provider);
            }
            return EffectiveAuthStore { store, sources };
        }

        let seed_store = self.seed_file.as_deref().map(AuthStore::load).unwrap_or_default();
        let runtime_store = self.runtime_file.as_deref().map(AuthStore::load).unwrap_or_default();

        let mut merged = seed_store.clone();
        merged.version = merged.version.max(runtime_store.version);

        let mut sources: HashMap<String, HashMap<String, AuthRecordSource>> = HashMap::new();
        for (provider, auth) in &seed_store.providers {
            let per_provider = sources.entry(provider.clone()).or_default();
            for account in auth.accounts.keys() {
                per_provider.insert(account.clone(), AuthRecordSource::Seed);
            }
        }

        for (provider, auth) in &runtime_store.providers {
            let merged_provider = merged.providers.entry(provider.clone()).or_default();
            if let Some(active_account) = &auth.active_account {
                merged_provider.active_account = Some(active_account.clone());
            }
            let per_provider = sources.entry(provider.clone()).or_default();
            for (account, credential) in &auth.accounts {
                merged_provider.accounts.insert(account.clone(), credential.clone());
                per_provider.insert(account.clone(), AuthRecordSource::Runtime);
            }
        }

        EffectiveAuthStore { store: merged, sources }
    }

    pub fn load_write_store(&self) -> AuthStore {
        self.write_path().map(AuthStore::load).unwrap_or_default()
    }

    pub fn save_write_store(&self, store: &AuthStore) -> crate::Result<()> {
        let path = self.write_path().ok_or_else(|| crate::Error::Auth {
            message: "no auth store write path configured".to_string(),
        })?;
        store.save(path)
    }

    pub fn mutate_write_store<F>(&self, mutate: F) -> crate::Result<()>
    where
        F: FnOnce(&mut AuthStore),
    {
        let mut store = self.load_write_store();
        mutate(&mut store);
        self.save_write_store(&store)
    }

    pub fn import_account(&self, export: &ProviderAccountExport, target: ImportTarget) -> crate::Result<()> {
        let path = match target {
            ImportTarget::Auto => self.write_path(),
            ImportTarget::File => self.auth_file.as_deref().or(self.write_path()),
            ImportTarget::Seed => self.seed_file.as_deref(),
            ImportTarget::Runtime => self.runtime_file.as_deref(),
        }
        .ok_or_else(|| crate::Error::Auth {
            message: format!("no {:?} auth store configured", target),
        })?;

        let mut store = AuthStore::load(path);
        store.set_credential(&export.provider, &export.account, export.credential.clone());
        if export.active || store.active_credential(&export.provider).is_none() {
            store.switch_account(&export.provider, &export.account);
        }
        store.save(path)
    }
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

const OPENAI_CODEX_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OPENAI_CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_CODEX_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const OPENAI_CODEX_SCOPE: &str = "openid profile email offline_access";
const OPENAI_CODEX_ACCOUNT_CLAIM: &str = "https://api.openai.com/auth";
const OPENAI_CODEX_ACCOUNT_ID_KEY: &str = "chatgpt_account_id";

#[derive(Debug)]
struct PkceChallenge {
    verifier: String,
    challenge: String,
}

fn generate_pkce() -> PkceChallenge {
    let mut verifier_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut verifier_bytes);

    let verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);

    let mut hasher = Sha256::new();
    hasher.update(verifier.as_bytes());
    let challenge = URL_SAFE_NO_PAD.encode(hasher.finalize());

    PkceChallenge { verifier, challenge }
}

fn expiration_from_expires_in(expires_in: i64) -> i64 {
    chrono::Utc::now().timestamp_millis() + (expires_in * 1000) - (5 * 60 * 1000)
}

#[derive(Debug, Deserialize)]
struct OpenAiCodexTokenResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

pub fn openai_codex_account_id_from_access_token(access_token: &str) -> crate::Result<String> {
    let mut parts = access_token.split('.');
    let _header = parts.next();
    let payload = parts.next().ok_or_else(|| crate::Error::Auth {
        message: "OpenAI Codex access token missing JWT payload".to_string(),
    })?;
    let _signature = parts.next().ok_or_else(|| crate::Error::Auth {
        message: "OpenAI Codex access token missing JWT signature".to_string(),
    })?;
    if parts.next().is_some() {
        return Err(crate::Error::Auth {
            message: "OpenAI Codex access token has too many JWT segments".to_string(),
        });
    }

    let payload_bytes = URL_SAFE_NO_PAD.decode(payload).map_err(|e| crate::Error::Auth {
        message: format!("Failed to decode OpenAI Codex JWT payload: {e}"),
    })?;
    let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes).map_err(|e| crate::Error::Auth {
        message: format!("Failed to parse OpenAI Codex JWT payload: {e}"),
    })?;

    payload_json
        .get(OPENAI_CODEX_ACCOUNT_CLAIM)
        .and_then(|value| value.get(OPENAI_CODEX_ACCOUNT_ID_KEY))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| crate::Error::Auth {
            message: "OpenAI Codex access token missing https://api.openai.com/auth.chatgpt_account_id".to_string(),
        })
}

pub fn openai_codex_account_id_from_credential(credential: &StoredCredential) -> crate::Result<String> {
    openai_codex_account_id_from_access_token(credential.token())
}

fn validate_openai_codex_tokens(tokens: &OpenAiCodexTokenResponse) -> crate::Result<()> {
    openai_codex_account_id_from_access_token(&tokens.access_token).map(|_| ())
}

fn openai_codex_code_exchange_form(code: &str, verifier: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "authorization_code".to_string()),
        ("client_id", OPENAI_CODEX_CLIENT_ID.to_string()),
        ("code", code.to_string()),
        ("code_verifier", verifier.to_string()),
        ("redirect_uri", OPENAI_CODEX_REDIRECT_URI.to_string()),
    ]
}

fn openai_codex_refresh_form(refresh_token: &str) -> Vec<(&'static str, String)> {
    vec![
        ("grant_type", "refresh_token".to_string()),
        ("client_id", OPENAI_CODEX_CLIENT_ID.to_string()),
        ("refresh_token", refresh_token.to_string()),
    ]
}

fn build_openai_codex_auth_url() -> crate::Result<(String, String)> {
    let pkce = generate_pkce();
    let url = url::Url::parse_with_params(
        OPENAI_CODEX_AUTHORIZE_URL,
        &[
            ("response_type", "code"),
            ("client_id", OPENAI_CODEX_CLIENT_ID),
            ("redirect_uri", OPENAI_CODEX_REDIRECT_URI),
            ("scope", OPENAI_CODEX_SCOPE),
            ("code_challenge", pkce.challenge.as_str()),
            ("code_challenge_method", "S256"),
            ("state", pkce.verifier.as_str()),
            ("id_token_add_organizations", "true"),
            ("codex_cli_simplified_flow", "true"),
            ("originator", "pi"),
        ],
    )
    .map_err(|e| crate::Error::Auth {
        message: format!("invalid OpenAI Codex authorization URL: {e}"),
    })?;
    Ok((url.to_string(), pkce.verifier))
}

async fn exchange_openai_codex_code(code: &str, verifier: &str) -> crate::Result<crate::oauth::OAuthCredentials> {
    let response = reqwest::Client::new()
        .post(OPENAI_CODEX_TOKEN_URL)
        .form(&openai_codex_code_exchange_form(code, verifier))
        .send()
        .await
        .map_err(|e| crate::Error::Auth {
            message: format!("failed to send OpenAI Codex token exchange request: {e}"),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(crate::Error::Auth {
            message: format!("OpenAI Codex token exchange failed ({status}): {error_text}"),
        });
    }

    let tokens: OpenAiCodexTokenResponse = response.json().await.map_err(|e| crate::Error::Auth {
        message: format!("failed to parse OpenAI Codex token exchange response: {e}"),
    })?;
    validate_openai_codex_tokens(&tokens)?;

    Ok(crate::oauth::OAuthCredentials {
        access: tokens.access_token,
        refresh: tokens.refresh_token,
        expires: expiration_from_expires_in(tokens.expires_in),
    })
}

async fn refresh_openai_codex_token(refresh_token: &str) -> crate::Result<crate::oauth::OAuthCredentials> {
    let response = reqwest::Client::new()
        .post(OPENAI_CODEX_TOKEN_URL)
        .form(&openai_codex_refresh_form(refresh_token))
        .send()
        .await
        .map_err(|e| crate::Error::Auth {
            message: format!("failed to send OpenAI Codex token refresh request: {e}"),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(crate::Error::Auth {
            message: format!("OpenAI Codex token refresh failed ({status}): {error_text}"),
        });
    }

    let tokens: OpenAiCodexTokenResponse = response.json().await.map_err(|e| crate::Error::Auth {
        message: format!("failed to parse OpenAI Codex token refresh response: {e}"),
    })?;
    validate_openai_codex_tokens(&tokens)?;

    Ok(crate::oauth::OAuthCredentials {
        access: tokens.access_token,
        refresh: tokens.refresh_token,
        expires: expiration_from_expires_in(tokens.expires_in),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OAuthFlow {
    Anthropic,
    OpenAiCodex,
}

impl OAuthFlow {
    pub fn provider_name(self) -> &'static str {
        match self {
            Self::Anthropic => "anthropic",
            Self::OpenAiCodex => "openai-codex",
        }
    }

    pub fn from_provider(provider: Option<&str>) -> crate::Result<Self> {
        match provider.unwrap_or("anthropic") {
            "anthropic" => Ok(Self::Anthropic),
            "openai-codex" => Ok(Self::OpenAiCodex),
            other => Err(crate::Error::Auth {
                message: format!(
                    "OAuth login is not supported for provider '{other}'. Supported OAuth providers: anthropic, openai-codex"
                ),
            }),
        }
    }

    pub fn build_auth_url(self) -> crate::Result<(String, String)> {
        match self {
            Self::Anthropic => Ok(crate::oauth::build_auth_url()),
            Self::OpenAiCodex => build_openai_codex_auth_url(),
        }
    }

    pub async fn exchange_code(
        self,
        code: &str,
        state: &str,
        verifier: &str,
    ) -> crate::Result<crate::oauth::OAuthCredentials> {
        match self {
            Self::Anthropic => crate::oauth::exchange_code(code, state, verifier).await,
            Self::OpenAiCodex => {
                let _ = state;
                exchange_openai_codex_code(code, verifier).await
            }
        }
    }

    pub async fn refresh_token(self, refresh_token: &str) -> crate::Result<crate::oauth::OAuthCredentials> {
        match self {
            Self::Anthropic => crate::oauth::refresh_token(refresh_token).await,
            Self::OpenAiCodex => refresh_openai_codex_token(refresh_token).await,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOAuthLogin {
    pub provider: String,
    pub account: String,
    pub verifier: String,
}

pub fn legacy_pending_oauth_login_path(base_dir: &Path) -> PathBuf {
    base_dir.join(".login_verifier")
}

pub fn pending_oauth_login_path(base_dir: &Path, provider: &str, account: &str) -> PathBuf {
    let provider_component: String = url::form_urlencoded::byte_serialize(provider.as_bytes()).collect();
    let account_component: String = url::form_urlencoded::byte_serialize(account.as_bytes()).collect();
    base_dir.join(".login_verifiers").join(provider_component).join(format!("{account_component}.json"))
}

impl PendingOAuthLogin {
    pub fn new(provider: impl Into<String>, account: impl Into<String>, verifier: impl Into<String>) -> Self {
        Self {
            provider: provider.into(),
            account: account.into(),
            verifier: verifier.into(),
        }
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    pub fn load(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&raw).ok().or_else(|| {
            let verifier = raw.trim();
            if verifier.is_empty() {
                None
            } else {
                Some(Self::new("anthropic", "default", verifier.to_string()))
            }
        })
    }
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

    #[cfg(unix)]
    #[test]
    fn test_save_sets_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let subdir = dir.path().join("secure");
        let path = subdir.join("auth.json");

        let mut store = AuthStore::default();
        store.set_credential("anthropic", "default", StoredCredential::ApiKey {
            api_key: "secret-key".into(),
            label: None,
        });
        store.save(&path).unwrap();

        let file_mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(file_mode, 0o600, "auth file should be owner-only read/write, got {:#o}", file_mode);

        let dir_mode = std::fs::metadata(&subdir).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700, "auth dir should be owner-only, got {:#o}", dir_mode);
    }

    #[cfg(unix)]
    #[test]
    fn test_save_tightens_existing_loose_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("auth.json");

        // Create with loose permissions first
        std::fs::write(&path, "{}").unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();

        let mut store = AuthStore::load(&path);
        store.set_credential("anthropic", "default", StoredCredential::ApiKey {
            api_key: "key".into(),
            label: None,
        });
        store.save(&path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "save should tighten loose permissions, got {:#o}", mode);
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
