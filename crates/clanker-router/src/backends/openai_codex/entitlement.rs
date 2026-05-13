use std::collections::HashMap;
#[cfg(test)]
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::time::Duration;

use serde_json::Value;
use serde_json::json;

use super::super::common;
use super::OPENAI_CODEX_BETA_HEADER;
use super::OPENAI_CODEX_MODEL_IDS;
use super::OPENAI_CODEX_NOT_ENTITLED_CODE;
use super::OPENAI_CODEX_PROBE_MODEL;
use super::OPENAI_CODEX_PROVIDER;
use super::OPENAI_CODEX_RESPONSES_URL;
use crate::auth::AuthStore;
use crate::auth::StoredCredential;
use crate::auth::openai_codex_account_id_from_credential;
use crate::credential::CredentialManager;
use crate::credential::OAuthTokens;
use crate::error::Result;
use crate::model::Model;
use crate::retry::RetryConfig;
use crate::retry::is_retryable_status;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntitlementState {
    Unknown,
    Entitled { checked_at_ms: i64 },
    NotEntitled { reason: String, checked_at_ms: i64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementRecord {
    pub state: EntitlementState,
    pub last_error: Option<String>,
}

impl Default for EntitlementRecord {
    fn default() -> Self {
        Self {
            state: EntitlementState::Unknown,
            last_error: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ProbeOutcome {
    Entitled,
    NotEntitled(String),
    Error(String),
}

fn entitlement_cache() -> &'static Mutex<HashMap<String, EntitlementRecord>> {
    static CACHE: OnceLock<Mutex<HashMap<String, EntitlementRecord>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_key(account: &str) -> String {
    format!("{OPENAI_CODEX_PROVIDER}:{account}")
}

pub(crate) fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

pub fn codex_models() -> Vec<Model> {
    OPENAI_CODEX_MODEL_IDS
        .iter()
        .map(|id| Model {
            id: (*id).to_string(),
            name: (*id).to_string(),
            provider: OPENAI_CODEX_PROVIDER.to_string(),
            max_input_tokens: 400_000,
            max_output_tokens: 128_000,
            supports_thinking: true,
            supports_images: true,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        })
        .collect()
}

pub fn entitlement_record(account: &str) -> EntitlementRecord {
    entitlement_cache()
        .lock()
        .expect("entitlement cache lock poisoned")
        .get(&cache_key(account))
        .cloned()
        .unwrap_or_default()
}

pub fn reset_entitlement(provider: &str, account: Option<&str>) {
    if provider != OPENAI_CODEX_PROVIDER {
        return;
    }

    let mut cache = entitlement_cache().lock().expect("entitlement cache lock poisoned");
    if let Some(account) = account {
        cache.remove(&cache_key(account));
    } else {
        cache.retain(|key, _| !key.starts_with(&format!("{OPENAI_CODEX_PROVIDER}:")));
    }
}

fn set_entitlement_record(account: &str, record: EntitlementRecord) -> EntitlementRecord {
    entitlement_cache()
        .lock()
        .expect("entitlement cache lock poisoned")
        .insert(cache_key(account), record.clone());
    record
}

pub(crate) fn classify_probe_response(status: u16, body: &str) -> ProbeOutcome {
    if (200..300).contains(&status) {
        return ProbeOutcome::Entitled;
    }

    let error_code = serde_json::from_str::<serde_json::Value>(body).ok().and_then(|value| {
        value
            .get("error")
            .and_then(|error| error.get("code"))
            .and_then(|code| code.as_str())
            .map(str::to_string)
    });

    if status == 403 || error_code.as_deref() == Some(OPENAI_CODEX_NOT_ENTITLED_CODE) {
        return ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string());
    }

    ProbeOutcome::Error(if body.trim().is_empty() {
        format!("entitlement probe failed with HTTP {status}")
    } else {
        format!("entitlement probe failed with HTTP {status}: {body}")
    })
}

#[cfg(test)]
pub(crate) type ProbeHook = Arc<dyn Fn(&StoredCredential) -> ProbeOutcome + Send + Sync>;

#[cfg(test)]
pub(crate) fn probe_hook() -> &'static Mutex<Option<ProbeHook>> {
    static HOOK: OnceLock<Mutex<Option<ProbeHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) type SleepHook = Arc<dyn Fn(Duration) + Send + Sync>;

#[cfg(test)]
pub(crate) fn sleep_hook() -> &'static Mutex<Option<SleepHook>> {
    static HOOK: OnceLock<Mutex<Option<SleepHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(crate) fn responses_url_override() -> &'static Mutex<Option<String>> {
    static OVERRIDE: OnceLock<Mutex<Option<String>>> = OnceLock::new();
    OVERRIDE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn responses_url() -> String {
    #[cfg(test)]
    if let Some(url) = responses_url_override().lock().expect("responses url override lock poisoned").clone() {
        return url;
    }

    OPENAI_CODEX_RESPONSES_URL.to_string()
}

pub(crate) async fn codex_sleep(duration: Duration) {
    #[cfg(test)]
    if let Some(hook) = sleep_hook().lock().expect("sleep hook lock poisoned").clone() {
        hook(duration);
        return;
    }

    tokio::time::sleep(duration).await;
}

pub(crate) fn build_probe_request_body() -> Value {
    json!({
        "model": OPENAI_CODEX_PROBE_MODEL,
        "store": false,
        "stream": true,
        "instructions": "codex entitlement probe",
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": "ping"}],
        }],
        "text": {"verbosity": "low"},
    })
}

pub(crate) fn build_probe_request(client: &reqwest::Client, credential: &StoredCredential) -> Result<reqwest::Request> {
    let token = credential.token().to_string();
    let account_id = openai_codex_account_id_from_credential(credential)?;

    client
        .post(responses_url())
        .header("authorization", format!("Bearer {token}"))
        .header("chatgpt-account-id", account_id)
        .header("OpenAI-Beta", OPENAI_CODEX_BETA_HEADER)
        .header("originator", "pi")
        .header("accept", "text/event-stream")
        .header("content-type", "application/json")
        .json(&build_probe_request_body())
        .build()
        .map_err(Into::into)
}

async fn send_probe_request(credential: &StoredCredential) -> Result<reqwest::Response> {
    let client = common::build_http_client(Duration::from_secs(30))?;
    let request = build_probe_request(&client, credential)?;
    client.execute(request).await.map_err(Into::into)
}

pub(crate) async fn live_probe(credential: &StoredCredential, manager: Option<&CredentialManager>) -> ProbeOutcome {
    let retry = RetryConfig::deterministic();
    let mut transient_attempt = 0;
    let mut did_refresh = false;
    let mut current = credential.clone();

    loop {
        let response = match send_probe_request(&current).await {
            Ok(response) => response,
            Err(e) => {
                if transient_attempt < retry.max_retries {
                    codex_sleep(retry.backoff_for(transient_attempt)).await;
                    transient_attempt += 1;
                    continue;
                }
                return ProbeOutcome::Error(format!("failed to send entitlement probe: {e}"));
            }
        };

        let status = response.status().as_u16();

        if status == 401 && !did_refresh {
            if let Some(manager) = manager {
                match manager.force_refresh().await {
                    Ok(refreshed) => {
                        current = refreshed;
                        did_refresh = true;
                        continue;
                    }
                    Err(e) => {
                        return ProbeOutcome::Error(format!("OpenAI Codex token refresh failed: {e}"));
                    }
                }
            }
            return ProbeOutcome::Error("OpenAI Codex account is unauthenticated".to_string());
        }

        if is_retryable_status(status) && transient_attempt < retry.max_retries {
            codex_sleep(retry.backoff_for(transient_attempt)).await;
            transient_attempt += 1;
            continue;
        }

        if (200..300).contains(&status) {
            return ProbeOutcome::Entitled;
        }

        let body = response.text().await.unwrap_or_default();
        return classify_probe_response(status, &body);
    }
}

async fn run_probe(credential: &StoredCredential, manager: Option<&CredentialManager>) -> ProbeOutcome {
    #[cfg(test)]
    if let Some(hook) = probe_hook().lock().expect("probe hook lock poisoned").clone() {
        return hook(credential);
    }

    live_probe(credential, manager).await
}

pub async fn ensure_entitlement(
    store: &AuthStore,
    account: &str,
    manager: Option<&CredentialManager>,
) -> EntitlementRecord {
    let cached = entitlement_record(account);
    match &cached.state {
        EntitlementState::Entitled { .. } | EntitlementState::NotEntitled { .. } => return cached,
        EntitlementState::Unknown if cached.last_error.is_some() => return cached,
        EntitlementState::Unknown => {}
    }

    let Some(mut credential) = store.credential_for(OPENAI_CODEX_PROVIDER, account).cloned() else {
        return cached;
    };
    if credential.is_expired() {
        if let Some(manager) = manager {
            match manager.get_credential().await {
                Ok(refreshed) => credential = refreshed,
                Err(e) => {
                    return set_entitlement_record(account, EntitlementRecord {
                        state: EntitlementState::Unknown,
                        last_error: Some(e.to_string()),
                    });
                }
            }
        } else {
            return cached;
        }
    }

    let checked_at_ms = now_ms();
    match run_probe(&credential, manager).await {
        ProbeOutcome::Entitled => set_entitlement_record(account, EntitlementRecord {
            state: EntitlementState::Entitled { checked_at_ms },
            last_error: None,
        }),
        ProbeOutcome::NotEntitled(reason) => set_entitlement_record(account, EntitlementRecord {
            state: EntitlementState::NotEntitled { reason, checked_at_ms },
            last_error: None,
        }),
        ProbeOutcome::Error(error) => set_entitlement_record(account, EntitlementRecord {
            state: EntitlementState::Unknown,
            last_error: Some(error),
        }),
    }
}

pub async fn codex_status_suffix(store: &AuthStore, account: &str) -> Option<String> {
    codex_status_suffix_with_manager(store, account, None).await
}

pub async fn codex_status_suffix_with_manager(
    store: &AuthStore,
    account: &str,
    manager: Option<&CredentialManager>,
) -> Option<String> {
    store.credential_for(OPENAI_CODEX_PROVIDER, account)?;

    let record = ensure_entitlement(store, account, manager).await;
    Some(match record.state {
        EntitlementState::Entitled { .. } => "codex entitled".to_string(),
        EntitlementState::NotEntitled { .. } => "authenticated but not entitled for Codex use".to_string(),
        EntitlementState::Unknown => {
            if record.last_error.is_some() {
                "authenticated, entitlement check failed".to_string()
            } else {
                "authenticated, entitlement unknown".to_string()
            }
        }
    })
}

pub async fn catalog_for_active_account(store: &AuthStore, account: &str) -> Vec<Model> {
    catalog_for_active_account_with_manager(store, account, None).await
}

pub async fn catalog_for_active_account_with_manager(
    store: &AuthStore,
    account: &str,
    manager: Option<&CredentialManager>,
) -> Vec<Model> {
    match ensure_entitlement(store, account, manager).await.state {
        EntitlementState::Entitled { .. } => codex_models(),
        EntitlementState::Unknown | EntitlementState::NotEntitled { .. } => Vec::new(),
    }
}

pub fn refresh_fn_for_codex()
-> impl Fn(&str) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<OAuthTokens>> + Send>> + Send + Sync + 'static
{
    |refresh_token| {
        let refresh_token = refresh_token.to_string();
        Box::pin(async move {
            let creds = crate::auth::OAuthFlow::OpenAiCodex.refresh_token(&refresh_token).await?;
            Ok(OAuthTokens {
                access_token: creds.access,
                refresh_token: creds.refresh,
                expires_at_ms: creds.expires,
            })
        })
    }
}
