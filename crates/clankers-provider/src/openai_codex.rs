#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::OnceLock;

use async_trait::async_trait;
use serde_json::json;
use tokio::sync::mpsc;

use crate::CompletionRequest;
use crate::Model;
use crate::Provider;
use crate::auth::StoredCredential;
use crate::auth::openai_codex_account_id_from_credential;
use crate::credential_manager::CredentialManager;
use crate::error::Result;
use crate::streaming::StreamEvent;

pub const OPENAI_CODEX_PROVIDER: &str = "openai-codex";
const OPENAI_CODEX_RESPONSES_URL: &str = "https://chatgpt.com/backend-api/codex/responses";
const OPENAI_CODEX_BETA_HEADER: &str = "responses=experimental";
const OPENAI_CODEX_NOT_ENTITLED_CODE: &str = "usage_not_included";

pub const OPENAI_CODEX_MODEL_IDS: [&str; 2] = ["gpt-5.3-codex", "gpt-5.3-codex-spark"];
const OPENAI_CODEX_PROBE_MODEL: &str = "gpt-5.3-codex";

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

fn now_ms() -> i64 {
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

fn classify_probe_response(status: u16, body: &str) -> ProbeOutcome {
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
type ProbeHook = Arc<dyn Fn(&StoredCredential) -> ProbeOutcome + Send + Sync>;

#[cfg(test)]
fn probe_hook() -> &'static Mutex<Option<ProbeHook>> {
    static HOOK: OnceLock<Mutex<Option<ProbeHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

fn run_live_probe(credential: &StoredCredential) -> ProbeOutcome {
    let token = credential.token().to_string();
    let account_id = match openai_codex_account_id_from_credential(credential) {
        Ok(id) => id,
        Err(e) => return ProbeOutcome::Error(e.to_string()),
    };

    std::thread::spawn(move || {
        let client = match reqwest::blocking::Client::builder().timeout(std::time::Duration::from_secs(10)).build() {
            Ok(client) => client,
            Err(e) => return ProbeOutcome::Error(format!("failed to build entitlement probe client: {e}")),
        };

        let response = match client
            .post(OPENAI_CODEX_RESPONSES_URL)
            .header("authorization", format!("Bearer {token}"))
            .header("chatgpt-account-id", account_id)
            .header("OpenAI-Beta", OPENAI_CODEX_BETA_HEADER)
            .header("originator", "pi")
            .header("accept", "text/event-stream")
            .header("content-type", "application/json")
            .json(&json!({
                "model": OPENAI_CODEX_PROBE_MODEL,
                "store": false,
                "stream": true,
                "instructions": "codex entitlement probe",
                "input": [{
                    "role": "user",
                    "content": [{"type": "input_text", "text": "ping"}],
                }],
                "text": {"verbosity": "low"},
            }))
            .send()
        {
            Ok(response) => response,
            Err(e) => return ProbeOutcome::Error(format!("failed to send entitlement probe: {e}")),
        };

        let status = response.status().as_u16();
        if (200..300).contains(&status) {
            ProbeOutcome::Entitled
        } else {
            let body = response.text().unwrap_or_default();
            classify_probe_response(status, &body)
        }
    })
    .join()
    .unwrap_or_else(|_| ProbeOutcome::Error("entitlement probe thread panicked".to_string()))
}

fn run_probe(credential: &StoredCredential) -> ProbeOutcome {
    #[cfg(test)]
    if let Some(hook) = probe_hook().lock().expect("probe hook lock poisoned").clone() {
        return hook(credential);
    }

    run_live_probe(credential)
}

pub fn ensure_entitlement(store: &crate::auth::AuthStore, account: &str) -> EntitlementRecord {
    let cached = entitlement_record(account);
    match &cached.state {
        EntitlementState::Entitled { .. } | EntitlementState::NotEntitled { .. } => return cached,
        EntitlementState::Unknown if cached.last_error.is_some() => return cached,
        EntitlementState::Unknown => {}
    }

    let Some(credential) = store.credential_for(OPENAI_CODEX_PROVIDER, account) else {
        return cached;
    };
    if credential.is_expired() {
        return cached;
    }

    let checked_at_ms = now_ms();
    match run_probe(credential) {
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

pub fn codex_status_suffix(store: &crate::auth::AuthStore, account: &str) -> Option<String> {
    let credential = store.credential_for(OPENAI_CODEX_PROVIDER, account)?;
    if credential.is_expired() {
        return None;
    }

    let record = ensure_entitlement(store, account);
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

pub fn catalog_for_active_account(store: &crate::auth::AuthStore, account: &str) -> Vec<Model> {
    match ensure_entitlement(store, account).state {
        EntitlementState::Entitled { .. } => codex_models(),
        EntitlementState::Unknown | EntitlementState::NotEntitled { .. } => Vec::new(),
    }
}

pub struct CodexStubProvider {
    credential_manager: Arc<CredentialManager>,
    models: Vec<Model>,
    account: String,
}

impl CodexStubProvider {
    pub fn new(credential_manager: Arc<CredentialManager>, models: Vec<Model>, account: String) -> Self {
        Self {
            credential_manager,
            models,
            account,
        }
    }
}

#[async_trait]
impl Provider for CodexStubProvider {
    async fn complete(&self, _request: CompletionRequest, _tx: mpsc::Sender<StreamEvent>) -> Result<()> {
        match entitlement_record(&self.account) {
            EntitlementRecord {
                state: EntitlementState::Entitled { .. },
                ..
            } => {
                return Err(crate::error::provider_err(
                    "openai-codex is authenticated and entitled, but the Codex Responses backend is not implemented yet",
                ));
            }
            EntitlementRecord {
                state: EntitlementState::NotEntitled { reason, .. },
                ..
            } => {
                return Err(crate::error::auth_err(format!(
                    "{reason}. ChatGPT Plus or Pro is required for openai-codex"
                )));
            }
            EntitlementRecord {
                state: EntitlementState::Unknown,
                last_error: Some(error),
            } => {
                let classified_message = format!("openai-codex entitlement check failed: {error}");
                return Err(crate::error::provider_err_with_status_for_provider(
                    503,
                    classified_message,
                    OPENAI_CODEX_PROVIDER,
                ));
            }
            EntitlementRecord {
                state: EntitlementState::Unknown,
                last_error: None,
            } => {}
        }

        let credential = self.credential_manager.get_credential().await?;
        let checked_at_ms = now_ms();
        match run_probe(&credential) {
            ProbeOutcome::Entitled => {
                set_entitlement_record(&self.account, EntitlementRecord {
                    state: EntitlementState::Entitled { checked_at_ms },
                    last_error: None,
                });
                Err(crate::error::provider_err(
                    "openai-codex is authenticated and entitled, but the Codex Responses backend is not implemented yet",
                ))
            }
            ProbeOutcome::NotEntitled(reason) => {
                set_entitlement_record(&self.account, EntitlementRecord {
                    state: EntitlementState::NotEntitled {
                        reason: reason.clone(),
                        checked_at_ms,
                    },
                    last_error: None,
                });
                Err(crate::error::auth_err(format!("{reason}. ChatGPT Plus or Pro is required for openai-codex")))
            }
            ProbeOutcome::Error(error) => {
                set_entitlement_record(&self.account, EntitlementRecord {
                    state: EntitlementState::Unknown,
                    last_error: Some(error.clone()),
                });
                let classified_message = format!("openai-codex entitlement check failed: {error}");
                Err(crate::error::provider_err_with_status_for_provider(503, classified_message, OPENAI_CODEX_PROVIDER))
            }
        }
    }

    fn models(&self) -> &[Model] {
        &self.models
    }

    fn name(&self) -> &str {
        OPENAI_CODEX_PROVIDER
    }

    async fn reload_credentials(&self) {
        reset_entitlement(OPENAI_CODEX_PROVIDER, None);
        self.credential_manager.reload_from_disk().await;
    }
}

#[cfg(test)]
fn entitlement_test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}

#[cfg(test)]
pub(crate) fn with_test_entitlement_state<R>(f: impl FnOnce() -> R) -> R {
    let _guard = entitlement_test_lock().lock().unwrap_or_else(|poison| poison.into_inner());
    reset_entitlement(OPENAI_CODEX_PROVIDER, None);
    let result = f();
    reset_entitlement(OPENAI_CODEX_PROVIDER, None);
    result
}

#[cfg(test)]
pub(crate) fn set_entitlement_record_for_test(account: &str, record: EntitlementRecord) {
    let _ = set_entitlement_record(account, record);
}

#[cfg(test)]
pub(crate) fn with_test_probe_hook<F, R>(hook: F, f: impl FnOnce() -> R) -> R
where F: Fn(&StoredCredential) -> ProbeOutcome + Send + Sync + 'static {
    with_test_entitlement_state(|| {
        *probe_hook().lock().expect("probe hook lock poisoned") = Some(Arc::new(hook));
        let result = f();
        *probe_hook().lock().expect("probe hook lock poisoned") = None;
        result
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use base64::Engine;

    use super::*;
    use crate::CompletionRequest;
    use crate::auth::AuthStoreExt;
    use crate::auth::OAuthCredentials;
    use crate::credential_manager::CredentialManager;

    fn codex_creds(account_id: &str) -> OAuthCredentials {
        OAuthCredentials {
            access: format!(
                "header.{}.signature",
                base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                    serde_json::json!({
                        "https://api.openai.com/auth": {
                            "chatgpt_account_id": account_id,
                        }
                    })
                    .to_string()
                    .as_bytes(),
                )
            ),
            refresh: "refresh".to_string(),
            expires: now_ms() + 3_600_000,
        }
    }

    fn codex_store() -> crate::auth::AuthStore {
        let mut store = crate::auth::AuthStore::default();
        store.set_provider_credentials(OPENAI_CODEX_PROVIDER, "work", codex_creds("acct-123"));
        store.switch_provider_account(OPENAI_CODEX_PROVIDER, "work");
        store
    }

    fn minimal_request() -> CompletionRequest {
        CompletionRequest {
            model: OPENAI_CODEX_MODEL_IDS[0].to_string(),
            messages: Vec::new(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            tools: Vec::new(),
            thinking: None,
            no_cache: false,
            cache_ttl: None,
            extra_params: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn codex_catalog_is_exact_fixed_set() {
        let ids: Vec<&str> = OPENAI_CODEX_MODEL_IDS.to_vec();
        let unique: HashSet<&str> = ids.iter().copied().collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(unique.len(), 2);
        assert_eq!(ids, vec!["gpt-5.3-codex", "gpt-5.3-codex-spark"]);
    }

    #[test]
    fn codex_status_suffix_reports_not_entitled() {
        with_test_probe_hook(
            |_| ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()),
            || {
                let store = codex_store();
                let suffix = codex_status_suffix(&store, "work").expect("suffix should exist");
                assert_eq!(suffix, "authenticated but not entitled for Codex use");
            },
        );
    }

    #[test]
    fn codex_status_suffix_reports_probe_failure() {
        with_test_probe_hook(
            |_| ProbeOutcome::Error("boom".to_string()),
            || {
                let store = codex_store();
                let suffix = codex_status_suffix(&store, "work").expect("suffix should exist");
                assert_eq!(suffix, "authenticated, entitlement check failed");
            },
        );
    }

    #[test]
    fn codex_status_suffix_triggers_probe_when_entitlement_is_unknown() {
        let probe_calls = Arc::new(AtomicUsize::new(0));
        with_test_probe_hook(
            {
                let probe_calls = Arc::clone(&probe_calls);
                move |_| {
                    probe_calls.fetch_add(1, Ordering::SeqCst);
                    ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string())
                }
            },
            || {
                let store = codex_store();
                let first = codex_status_suffix(&store, "work").expect("suffix should exist");
                let second = codex_status_suffix(&store, "work").expect("suffix should exist");
                assert_eq!(first, "authenticated but not entitled for Codex use");
                assert_eq!(second, "authenticated but not entitled for Codex use");
                assert_eq!(probe_calls.load(Ordering::SeqCst), 1);
            },
        );
    }

    #[test]
    fn codex_catalog_requires_entitlement() {
        with_test_probe_hook(
            |_| ProbeOutcome::Entitled,
            || {
                let store = codex_store();
                let models = catalog_for_active_account(&store, "work");
                let ids: Vec<String> = models.into_iter().map(|m| m.id).collect();
                assert_eq!(ids, OPENAI_CODEX_MODEL_IDS.iter().map(|id| id.to_string()).collect::<Vec<_>>());
            },
        );

        with_test_probe_hook(
            |_| ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()),
            || {
                let store = codex_store();
                assert!(catalog_for_active_account(&store, "work").is_empty());
            },
        );
    }

    #[test]
    fn classify_probe_response_treats_usage_not_included_as_not_entitled() {
        let outcome = classify_probe_response(400, r#"{"error":{"code":"usage_not_included"}}"#);
        assert_eq!(outcome, ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()));
    }

    #[test]
    fn classify_probe_response_treats_http_403_as_not_entitled() {
        let outcome = classify_probe_response(403, "forbidden");
        assert_eq!(outcome, ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string()));
    }

    #[test]
    fn codex_reload_resets_entitlement_and_reprobes() {
        with_test_probe_hook(
            |_| ProbeOutcome::Entitled,
            || {
                let dir = tempfile::TempDir::new().unwrap();
                let auth_path = dir.path().join("auth.json");
                let mut store = crate::auth::AuthStore::default();
                store.set_provider_credentials(OPENAI_CODEX_PROVIDER, "work", codex_creds("acct-reloaded"));
                assert!(store.switch_provider_account(OPENAI_CODEX_PROVIDER, "work"));
                store.save(&auth_path).unwrap();

                set_entitlement_record_for_test("work", EntitlementRecord {
                    state: EntitlementState::NotEntitled {
                        reason: "authenticated but not entitled for Codex use".to_string(),
                        checked_at_ms: 1,
                    },
                    last_error: None,
                });

                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                runtime.block_on(async {
                    let manager = CredentialManager::new_for_provider(
                        OPENAI_CODEX_PROVIDER,
                        codex_creds("acct-stale").to_stored(),
                        auth_path.clone(),
                        None,
                    );
                    let provider = CodexStubProvider::new(manager, Vec::new(), "work".to_string());
                    provider.reload_credentials().await;
                });

                assert!(matches!(entitlement_record("work").state, EntitlementState::Unknown));
                let reloaded_store = crate::auth::AuthStore::load(&auth_path);
                assert_eq!(codex_status_suffix(&reloaded_store, "work"), Some("codex entitled".to_string()));
            },
        );
    }

    #[test]
    fn codex_account_switch_resets_entitlement_and_reprobes_new_account() {
        with_test_probe_hook(
            |credential| {
                let account_id = crate::auth::openai_codex_account_id_from_credential(credential).unwrap();
                if account_id == "acct-work" {
                    ProbeOutcome::NotEntitled("authenticated but not entitled for Codex use".to_string())
                } else {
                    ProbeOutcome::Entitled
                }
            },
            || {
                let mut store = crate::auth::AuthStore::default();
                store.set_provider_credentials(OPENAI_CODEX_PROVIDER, "work", codex_creds("acct-work"));
                store.set_provider_credentials(OPENAI_CODEX_PROVIDER, "backup", codex_creds("acct-backup"));
                assert!(store.switch_provider_account(OPENAI_CODEX_PROVIDER, "work"));

                assert_eq!(
                    codex_status_suffix(&store, "work"),
                    Some("authenticated but not entitled for Codex use".to_string())
                );
                assert!(matches!(entitlement_record("work").state, EntitlementState::NotEntitled { .. }));

                assert!(store.switch_provider_account(OPENAI_CODEX_PROVIDER, "backup"));
                reset_entitlement(OPENAI_CODEX_PROVIDER, None);

                assert_eq!(codex_status_suffix(&store, "backup"), Some("codex entitled".to_string()));
                assert!(matches!(entitlement_record("backup").state, EntitlementState::Entitled { .. }));
            },
        );
    }

    #[test]
    fn codex_complete_blocks_request_without_valid_account_id() {
        with_test_entitlement_state(|| {
            let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
            runtime.block_on(async {
                let dir = tempfile::TempDir::new().unwrap();
                let auth_path = dir.path().join("auth.json");
                let manager = CredentialManager::new_for_provider(
                    OPENAI_CODEX_PROVIDER,
                    crate::auth::StoredCredential::OAuth {
                        access_token: "not-a-jwt".to_string(),
                        refresh_token: "refresh".to_string(),
                        expires_at_ms: now_ms() + 3_600_000,
                        label: None,
                    },
                    auth_path,
                    None,
                );
                let provider = CodexStubProvider::new(manager, codex_models(), "work".to_string());
                let (tx, _rx) = tokio::sync::mpsc::channel(1);
                let err = provider.complete(minimal_request(), tx).await.unwrap_err();
                assert!(err.message.contains("entitlement check failed"));
                assert!(
                    err.message.contains("OpenAI Codex access token") || err.message.contains("chatgpt_account_id")
                );
            });
        });
    }
}
