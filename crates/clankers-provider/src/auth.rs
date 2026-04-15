//! Auth — delegates to `clanker_router::auth` and `clanker_router::oauth`
//!
//! All credential storage uses clanker-router's multi-provider auth store
//! at `~/.config/clanker-router/auth.json`.

use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rand::RngCore;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

// Re-export the canonical types from clanker-router
pub use clanker_router::auth::{
    AccountInfo, AuthStore, ProviderAuth, StoredCredential, env_var_for_provider, is_oauth_token,
};
pub use clanker_router::oauth::OAuthCredentials;

/// Default OAuth provider when the user omits `--provider` or `/login <provider>`.
pub const DEFAULT_OAUTH_PROVIDER: &str = "anthropic";

const OPENAI_CODEX_AUTHORIZE_URL: &str = "https://auth.openai.com/oauth/authorize";
const OPENAI_CODEX_TOKEN_URL: &str = "https://auth.openai.com/oauth/token";
const OPENAI_CODEX_CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
const OPENAI_CODEX_REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
const OPENAI_CODEX_SCOPE: &str = "openid profile email offline_access";
const OPENAI_CODEX_ACCOUNT_CLAIM: &str = "https://api.openai.com/auth";
const OPENAI_CODEX_ACCOUNT_ID_KEY: &str = "chatgpt_account_id";

/// Resolved credential for making API calls.
pub type Credential = StoredCredential;

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

pub fn openai_codex_account_id_from_access_token(access_token: &str) -> crate::error::Result<String> {
    let mut parts = access_token.split('.');
    let _header = parts.next();
    let payload = parts.next().ok_or_else(|| crate::error::auth_err("OpenAI Codex access token missing JWT payload"))?;
    let _signature = parts.next().ok_or_else(|| crate::error::auth_err("OpenAI Codex access token missing JWT signature"))?;
    if parts.next().is_some() {
        return Err(crate::error::auth_err("OpenAI Codex access token has too many JWT segments"));
    }

    let payload_bytes = URL_SAFE_NO_PAD
        .decode(payload)
        .map_err(|e| crate::error::auth_err(format!("Failed to decode OpenAI Codex JWT payload: {e}")))?;
    let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)
        .map_err(|e| crate::error::auth_err(format!("Failed to parse OpenAI Codex JWT payload: {e}")))?;

    payload_json
        .get(OPENAI_CODEX_ACCOUNT_CLAIM)
        .and_then(|value| value.get(OPENAI_CODEX_ACCOUNT_ID_KEY))
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| crate::error::auth_err("OpenAI Codex access token missing https://api.openai.com/auth.chatgpt_account_id"))
}

pub fn openai_codex_account_id_from_credential(credential: &StoredCredential) -> crate::error::Result<String> {
    openai_codex_account_id_from_access_token(credential.token())
}

fn validate_openai_codex_tokens(tokens: &OpenAiCodexTokenResponse) -> crate::error::Result<()> {
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

fn build_openai_codex_auth_url() -> (String, String) {
    let pkce = generate_pkce();
    let url = url::Url::parse_with_params(OPENAI_CODEX_AUTHORIZE_URL, &[
        ("response_type", "code"),
        ("client_id", OPENAI_CODEX_CLIENT_ID),
        ("redirect_uri", OPENAI_CODEX_REDIRECT_URI),
        ("scope", OPENAI_CODEX_SCOPE),
        ("code_challenge", &pkce.challenge),
        ("code_challenge_method", "S256"),
        ("state", &pkce.verifier),
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
        ("originator", "pi"),
    ])
    .expect("valid OpenAI Codex authorization URL");

    (url.to_string(), pkce.verifier)
}

async fn exchange_openai_codex_code(code: &str, verifier: &str) -> crate::error::Result<OAuthCredentials> {
    let response = reqwest::Client::new()
        .post(OPENAI_CODEX_TOKEN_URL)
        .form(&openai_codex_code_exchange_form(code, verifier))
        .send()
        .await
        .map_err(|e| crate::error::auth_err(format!("failed to send OpenAI Codex token exchange request: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(crate::error::auth_err(format!(
            "OpenAI Codex token exchange failed ({status}): {error_text}"
        )));
    }

    let tokens: OpenAiCodexTokenResponse = response
        .json()
        .await
        .map_err(|e| crate::error::auth_err(format!("failed to parse OpenAI Codex token exchange response: {e}")))?;
    validate_openai_codex_tokens(&tokens)?;

    Ok(OAuthCredentials {
        access: tokens.access_token,
        refresh: tokens.refresh_token,
        expires: expiration_from_expires_in(tokens.expires_in),
    })
}

async fn refresh_openai_codex_token(refresh_token: &str) -> crate::error::Result<OAuthCredentials> {
    let response = reqwest::Client::new()
        .post(OPENAI_CODEX_TOKEN_URL)
        .form(&openai_codex_refresh_form(refresh_token))
        .send()
        .await
        .map_err(|e| crate::error::auth_err(format!("failed to send OpenAI Codex token refresh request: {e}")))?;

    if !response.status().is_success() {
        let status = response.status();
        let error_text = response.text().await.unwrap_or_else(|_| "unknown error".to_string());
        return Err(crate::error::auth_err(format!(
            "OpenAI Codex token refresh failed ({status}): {error_text}"
        )));
    }

    let tokens: OpenAiCodexTokenResponse = response
        .json()
        .await
        .map_err(|e| crate::error::auth_err(format!("failed to parse OpenAI Codex token refresh response: {e}")))?;
    validate_openai_codex_tokens(&tokens)?;

    Ok(OAuthCredentials {
        access: tokens.access_token,
        refresh: tokens.refresh_token,
        expires: expiration_from_expires_in(tokens.expires_in),
    })
}

/// Provider-aware OAuth driver selection.
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

    pub fn from_provider(provider: Option<&str>) -> crate::error::Result<Self> {
        match provider.unwrap_or(DEFAULT_OAUTH_PROVIDER) {
            "anthropic" => Ok(Self::Anthropic),
            "openai-codex" => Ok(Self::OpenAiCodex),
            other => Err(crate::error::auth_err(format!(
                "OAuth login is not supported for provider '{other}'. Supported OAuth providers: anthropic, openai-codex"
            ))),
        }
    }

    pub fn build_auth_url(self) -> crate::error::Result<(String, String)> {
        match self {
            Self::Anthropic => Ok(clanker_router::oauth::build_auth_url()),
            Self::OpenAiCodex => Ok(build_openai_codex_auth_url()),
        }
    }

    pub async fn exchange_code(self, code: &str, state: &str, verifier: &str) -> crate::error::Result<OAuthCredentials> {
        match self {
            Self::Anthropic => clanker_router::oauth::exchange_code(code, state, verifier).await.map_err(Into::into),
            Self::OpenAiCodex => {
                let _ = state;
                exchange_openai_codex_code(code, verifier).await
            }
        }
    }

    pub async fn refresh_token(self, refresh_token: &str) -> crate::error::Result<OAuthCredentials> {
        match self {
            Self::Anthropic => clanker_router::oauth::refresh_token(refresh_token).await.map_err(Into::into),
            Self::OpenAiCodex => refresh_openai_codex_token(refresh_token).await,
        }
    }
}

/// Pending OAuth login persisted between login start and code exchange.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PendingOAuthLogin {
    pub provider: String,
    pub account: String,
    pub verifier: String,
}

pub fn legacy_pending_oauth_login_path(base_dir: &Path) -> std::path::PathBuf {
    base_dir.join(".login_verifier")
}

pub fn pending_oauth_login_path(base_dir: &Path, provider: &str, account: &str) -> std::path::PathBuf {
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
                Some(Self::new(DEFAULT_OAUTH_PROVIDER, "default", verifier.to_string()))
            }
        })
    }
}

/// Anthropic-specific convenience methods on [`AuthStore`].
///
/// Generic provider-aware helpers live here too so commands and slash commands
/// can switch from hardcoded Anthropic behavior incrementally.
pub trait AuthStoreExt {
    fn active_account_name_for(&self, provider: &str) -> &str;
    fn active_oauth_credentials_for(&self, provider: &str) -> Option<OAuthCredentials>;
    fn set_provider_credentials(&mut self, provider: &str, account: &str, creds: OAuthCredentials);
    fn switch_provider_account(&mut self, provider: &str, account: &str) -> bool;
    fn remove_provider_account(&mut self, provider: &str, account: &str) -> bool;
    fn list_provider_accounts(&self, provider: &str) -> Vec<AccountInfo>;
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
    fn active_account_name_for(&self, provider: &str) -> &str {
        self.providers.get(provider).and_then(|p| p.active_account.as_deref()).unwrap_or("default")
    }

    fn active_oauth_credentials_for(&self, provider: &str) -> Option<OAuthCredentials> {
        let cred = self.active_credential(provider)?;
        OAuthCredentials::from_stored(cred)
    }

    fn set_provider_credentials(&mut self, provider: &str, account: &str, creds: OAuthCredentials) {
        self.set_credential(provider, account, creds.to_stored());
    }

    fn switch_provider_account(&mut self, provider: &str, account: &str) -> bool {
        clanker_router::auth::AuthStore::switch_account(self, provider, account)
    }

    fn remove_provider_account(&mut self, provider: &str, account: &str) -> bool {
        clanker_router::auth::AuthStore::remove_account(self, provider, account)
    }

    fn list_provider_accounts(&self, provider: &str) -> Vec<AccountInfo> {
        self.list_accounts(provider)
    }

    fn active_account_name(&self) -> &str {
        self.active_account_name_for("anthropic")
    }

    fn active_credentials(&self) -> Option<OAuthCredentials> {
        self.active_oauth_credentials_for("anthropic")
    }

    fn set_credentials(&mut self, account: &str, creds: OAuthCredentials) {
        self.set_provider_credentials("anthropic", account, creds);
    }

    fn switch_anthropic_account(&mut self, account: &str) -> bool {
        self.switch_provider_account("anthropic", account)
    }

    fn remove_anthropic_account(&mut self, account: &str) -> bool {
        self.remove_provider_account("anthropic", account)
    }

    fn list_anthropic_accounts(&self) -> Vec<AccountInfo> {
        self.list_provider_accounts("anthropic")
    }

    fn account_summary(&self) -> String {
        use std::fmt::Write;
        let accounts = self.list_anthropic_accounts();
        if accounts.is_empty() {
            return "No accounts configured. Use /login or `clankers auth login` to add one.".to_string();
        }
        let mut out = String::new();
        for info in &accounts {
            let marker = if info.is_active { "▸" } else { " " };
            let status = if info.is_expired { " (expired)" } else { "" };
            let label = info.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
            writeln!(out, "{} {}{}{}", marker, info.name, label, status).ok();
        }
        out
    }

    fn save_clankers(&self, path: &Path) -> crate::error::Result<()> {
        self.save(path).map_err(|e| e.into())
    }
}

/// Resolve credentials for API access (Anthropic).
///
/// Delegates to `clanker_router::auth::resolve_credential`.
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
    resolve_provider_credential_with_fallback(
        "anthropic",
        runtime_override,
        auth_store_path,
        fallback_auth_path,
        account,
    )
}

/// Resolve provider credentials with fallback and account selection.
pub fn resolve_provider_credential_with_fallback(
    provider: &str,
    runtime_override: Option<&str>,
    auth_store_path: &Path,
    fallback_auth_path: Option<&Path>,
    account: Option<&str>,
) -> Option<Credential> {
    let store = AuthStore::load(auth_store_path);
    let fallback = fallback_auth_path.map(AuthStore::load);

    if let Some(acct) = account
        && let Some(cred) = store.credential_for(provider, acct)
    {
        return Some(cred.clone());
    }

    clanker_router::auth::resolve_credential(provider, runtime_override, &store, fallback.as_ref())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_auth_store_ext_active_account() {
        let store = AuthStore::default();
        assert_eq!(store.active_account_name(), "default");
        assert_eq!(store.active_account_name_for("openai-codex"), "default");
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
    fn test_credential_expired() {
        let api_key = StoredCredential::ApiKey {
            api_key: "sk-test".into(),
            label: None,
        };
        assert!(!api_key.is_expired());

        let expired_oauth = StoredCredential::OAuth {
            access_token: "t".into(),
            refresh_token: "r".into(),
            expires_at_ms: 0,
            label: None,
        };
        assert!(expired_oauth.is_expired());
    }

    fn fake_openai_codex_jwt(account_id: &str) -> String {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload_json = serde_json::json!({
            OPENAI_CODEX_ACCOUNT_CLAIM: {
                OPENAI_CODEX_ACCOUNT_ID_KEY: account_id,
            }
        })
        .to_string();
        let payload = URL_SAFE_NO_PAD.encode(payload_json.as_bytes());
        format!("{header}.{payload}.sig")
    }

    #[test]
    fn test_oauth_flow_defaults_to_anthropic() {
        assert_eq!(OAuthFlow::from_provider(None).unwrap().provider_name(), "anthropic");
    }

    #[test]
    fn test_openai_codex_auth_url_contains_required_contract() {
        let (url, verifier) = OAuthFlow::OpenAiCodex.build_auth_url().unwrap();
        assert!(url.starts_with(OPENAI_CODEX_AUTHORIZE_URL));
        assert!(url.contains("response_type=code"));
        assert!(url.contains(&format!("client_id={OPENAI_CODEX_CLIENT_ID}")));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
        assert!(url.contains("id_token_add_organizations=true"));
        assert!(url.contains("codex_cli_simplified_flow=true"));
        assert!(url.contains("originator=pi"));
        assert!(!verifier.is_empty());
    }

    #[test]
    fn test_openai_codex_token_contract_helpers() {
        let exchange = openai_codex_code_exchange_form("code-123", "verifier-456");
        assert_eq!(OPENAI_CODEX_TOKEN_URL, "https://auth.openai.com/oauth/token");
        assert_eq!(
            exchange,
            vec![
                ("grant_type", "authorization_code".to_string()),
                ("client_id", OPENAI_CODEX_CLIENT_ID.to_string()),
                ("code", "code-123".to_string()),
                ("code_verifier", "verifier-456".to_string()),
                ("redirect_uri", OPENAI_CODEX_REDIRECT_URI.to_string()),
            ]
        );

        let refresh = openai_codex_refresh_form("refresh-789");
        assert_eq!(
            refresh,
            vec![
                ("grant_type", "refresh_token".to_string()),
                ("client_id", OPENAI_CODEX_CLIENT_ID.to_string()),
                ("refresh_token", "refresh-789".to_string()),
            ]
        );
    }

    #[test]
    fn test_openai_codex_account_id_derives_from_access_token() {
        let token = fake_openai_codex_jwt("acct_123");
        assert_eq!(openai_codex_account_id_from_access_token(&token).unwrap(), "acct_123");
    }

    #[test]
    fn test_openai_codex_account_id_rejects_missing_claim() {
        let header = URL_SAFE_NO_PAD.encode(r#"{"alg":"none","typ":"JWT"}"#);
        let payload = URL_SAFE_NO_PAD.encode(r#"{"sub":"user_123"}"#);
        let token = format!("{header}.{payload}.sig");
        assert!(openai_codex_account_id_from_access_token(&token).is_err());
    }

    #[test]
    fn test_pending_oauth_login_roundtrip() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("pending.json");
        let pending = PendingOAuthLogin::new("anthropic", "work", "verifier-123");
        pending.save(&path).unwrap();
        assert_eq!(PendingOAuthLogin::load(&path), Some(pending));
    }

    #[test]
    fn test_pending_oauth_login_paths_are_isolated_by_provider_and_account() {
        let dir = tempfile::TempDir::new().unwrap();
        let anthropic_default = pending_oauth_login_path(dir.path(), "anthropic", "default");
        let codex_default = pending_oauth_login_path(dir.path(), "openai-codex", "default");
        let codex_work = pending_oauth_login_path(dir.path(), "openai-codex", "work");

        assert_ne!(anthropic_default, codex_default);
        assert_ne!(codex_default, codex_work);

        let pending_a = PendingOAuthLogin::new("anthropic", "default", "verify-a");
        let pending_b = PendingOAuthLogin::new("openai-codex", "work", "verify-b");
        pending_a.save(&anthropic_default).unwrap();
        pending_b.save(&codex_work).unwrap();

        assert_eq!(PendingOAuthLogin::load(&anthropic_default), Some(pending_a));
        assert_eq!(PendingOAuthLogin::load(&codex_work), Some(pending_b));
    }

    #[test]
    fn test_openai_codex_login_persistence_preserves_other_providers() {
        let dir = tempfile::TempDir::new().unwrap();
        let auth_path = dir.path().join("auth.json");

        let mut store = AuthStore::default();
        store.set_provider_credentials(
            "anthropic",
            "default",
            OAuthCredentials {
                access: "anthropic-access".into(),
                refresh: "anthropic-refresh".into(),
                expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
            },
        );
        store.set_credential(
            "openai",
            "default",
            StoredCredential::ApiKey {
                api_key: "sk-openai".into(),
                label: None,
            },
        );
        store.save(&auth_path).unwrap();

        let mut reloaded = AuthStore::load(&auth_path);
        let codex_access = fake_openai_codex_jwt("acct_codex");
        reloaded.set_provider_credentials(
            "openai-codex",
            "work",
            OAuthCredentials {
                access: codex_access.clone(),
                refresh: "codex-refresh".into(),
                expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
            },
        );
        assert!(reloaded.switch_provider_account("openai-codex", "work"));
        reloaded.save(&auth_path).unwrap();

        let final_store = AuthStore::load(&auth_path);
        assert_eq!(final_store.credential_for("anthropic", "default").unwrap().token(), "anthropic-access");
        assert_eq!(final_store.credential_for("openai", "default").unwrap().token(), "sk-openai");
        assert_eq!(final_store.credential_for("openai-codex", "work").unwrap().token(), codex_access);
        assert_eq!(final_store.active_account_name_for("openai-codex"), "work");
    }

    #[test]
    fn test_pending_oauth_login_legacy_verifier_defaults_to_anthropic() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("pending.txt");
        std::fs::write(&path, "legacy-verifier").unwrap();
        assert_eq!(
            PendingOAuthLogin::load(&path),
            Some(PendingOAuthLogin::new("anthropic", "default", "legacy-verifier"))
        );
    }
}
