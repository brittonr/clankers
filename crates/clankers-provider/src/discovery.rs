//! Multi-provider router discovery and construction.
//!
//! Builds a [`RouterProvider`](super::router::RouterProvider) by probing for
//! available API credentials (env vars, auth store, Ollama) and optionally
//! connecting to a running `clanker-router` daemon over iroh RPC.

use std::sync::Arc;

use tracing::info;

use super::Provider;
use super::anthropic::AnthropicProvider;
use super::auth;
use super::credential_manager::CredentialManager;
use super::openai_codex;
use crate::auth::AuthStoreExt;
use crate::CompletionRequest;
use crate::Model;
use crate::auth::AuthStore;
use crate::error::Result;
use crate::streaming::StreamEvent;

/// Build a multi-provider router that auto-discovers available providers.
///
/// Resolution order:
/// 1. Try connecting to a running clanker-router daemon (iroh RPC)
/// 2. Try auto-starting the daemon if `clanker-router` is in PATH
/// 3. Fall back to in-process provider construction
///
/// In-process discovers providers from:
/// - Anthropic (ANTHROPIC_API_KEY, OAuth, or auth.json)
/// - OpenAI (OPENAI_API_KEY)
/// - OpenRouter (OPENROUTER_API_KEY)
/// - Groq (GROQ_API_KEY)
/// - DeepSeek (DEEPSEEK_API_KEY)
pub async fn build_router_with_rpc(
    api_key_override: Option<&str>,
    base_url: Option<String>,
    auth_store_path: &std::path::Path,
    fallback_auth_path: Option<&std::path::Path>,
    account: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    use super::rpc_provider::RpcProvider;

    // Skip RPC if CLANKERS_NO_DAEMON is set (useful for testing/debugging).
    // The extracted clanker-router now owns the Codex backend too, so
    // service and local daemon paths can share the same routed provider set.
    if std::env::var("CLANKERS_NO_DAEMON").is_err()
        && let Some(provider) = RpcProvider::auto_start_and_connect().await
    {
        return Ok(provider);
    }

    // Fall back to in-process
    build_router(api_key_override, base_url, auth_store_path, fallback_auth_path, account)
}

fn select_codex_store_and_account<'a>(
    primary_store: &'a AuthStore,
    fallback_store: Option<&'a AuthStore>,
    account: Option<&str>,
) -> (&'a AuthStore, String) {
    if let Some(account) = account {
        if primary_store
            .credential_for(openai_codex::OPENAI_CODEX_PROVIDER, account)
            .is_some()
        {
            return (primary_store, account.to_string());
        }
        if let Some(fallback_store) = fallback_store
            && fallback_store
                .credential_for(openai_codex::OPENAI_CODEX_PROVIDER, account)
                .is_some()
        {
            return (fallback_store, account.to_string());
        }
        return (primary_store, account.to_string());
    }

    if primary_store
        .active_credential(openai_codex::OPENAI_CODEX_PROVIDER)
        .is_some()
    {
        return (
            primary_store,
            primary_store
                .active_account_name_for(openai_codex::OPENAI_CODEX_PROVIDER)
                .to_string(),
        );
    }

    if let Some(fallback_store) = fallback_store
        && fallback_store
            .active_credential(openai_codex::OPENAI_CODEX_PROVIDER)
            .is_some()
    {
        return (
            fallback_store,
            fallback_store
                .active_account_name_for(openai_codex::OPENAI_CODEX_PROVIDER)
                .to_string(),
        );
    }

    (
        primary_store,
        primary_store
            .active_account_name_for(openai_codex::OPENAI_CODEX_PROVIDER)
            .to_string(),
    )
}

/// Build in-process (no RPC). Used as fallback when daemon is unavailable.
#[cfg_attr(dylint_lib = "tigerstyle", allow(function_length, reason = "sequential setup/dispatch logic"))]
pub fn build_router(
    api_key_override: Option<&str>,
    base_url: Option<String>,
    auth_store_path: &std::path::Path,
    fallback_auth_path: Option<&std::path::Path>,
    account: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    use clanker_router::backends::openai_compat::OpenAICompatConfig;
    use clanker_router::backends::openai_compat::OpenAICompatProvider;

    use super::router::RouterCompatAdapter;
    use super::router::RouterProvider;

    let mut backends: Vec<(String, Arc<dyn Provider>)> = Vec::new();

    // 1. Anthropic (OAuth + API key + env var)
    //
    // Collect all available credentials. When multiple accounts exist,
    // build a CredentialPool with failover — if one account gets
    // rate-limited, the provider automatically tries the next.
    let anthropic_cred =
        auth::resolve_credential_with_fallback(api_key_override, auth_store_path, fallback_auth_path, account);

    if let Some(credential) = anthropic_cred {
        // Check for additional accounts in the auth store
        let store = clanker_router::auth::AuthStore::load(auth_store_path);
        let all_creds = store.all_credentials("anthropic");

        let provider: Arc<dyn Provider> = if credential.is_oauth()
            || clanker_router::auth::is_oauth_token(credential.token())
        {
            let cm = CredentialManager::new(
                credential,
                auth_store_path.to_path_buf(),
                fallback_auth_path.map(|p| p.to_path_buf()),
            );

            if all_creds.len() > 1 {
                // Multi-account: build a credential pool for failover
                let pool_creds: Vec<(String, clanker_router::auth::StoredCredential)> = all_creds;
                info!(
                    "Anthropic: {} account(s) available, enabling credential pool failover",
                    pool_creds.len()
                );
                let pool = clanker_router::credential_pool::CredentialPool::new(
                    pool_creds,
                    clanker_router::credential_pool::SelectionStrategy::Failover,
                );
                Arc::new(AnthropicProvider::with_credential_pool(cm, pool, base_url))
            } else {
                Arc::new(AnthropicProvider::with_credential_manager(cm, base_url))
            }
        } else {
            Arc::new(AnthropicProvider::new(credential, base_url))
        };
        backends.push(("anthropic".to_string(), provider));
    }

    // 2. OpenAI Codex subscription provider (OAuth only)
    if let Some(credential) = auth::resolve_provider_credential_with_fallback(
        openai_codex::OPENAI_CODEX_PROVIDER,
        None,
        auth_store_path,
        fallback_auth_path,
        account,
    ) {
        let primary_store = AuthStore::load(auth_store_path);
        let fallback_store = fallback_auth_path.map(AuthStore::load);
        let (catalog_store, account_name) =
            select_codex_store_and_account(&primary_store, fallback_store.as_ref(), account);
        let models = openai_codex::catalog_for_active_account(catalog_store, &account_name);
        let cm = CredentialManager::new_for_provider(
            openai_codex::OPENAI_CODEX_PROVIDER,
            credential,
            auth_store_path.to_path_buf(),
            fallback_auth_path.map(|p| p.to_path_buf()),
        );
        let provider: Arc<dyn Provider> = Arc::new(openai_codex::CodexStubProvider::new(cm, models, account_name));
        backends.push((openai_codex::OPENAI_CODEX_PROVIDER.to_string(), provider));
    }

    // 3. OpenAI-compatible providers from env vars
    type CompatFactory = fn(String) -> OpenAICompatConfig;
    let compat_providers: &[(&str, CompatFactory)] = &[
        ("OPENAI_API_KEY", OpenAICompatConfig::openai),
        ("OPENROUTER_API_KEY", OpenAICompatConfig::openrouter),
        ("GROQ_API_KEY", OpenAICompatConfig::groq),
        ("DEEPSEEK_API_KEY", OpenAICompatConfig::deepseek),
        ("MISTRAL_API_KEY", OpenAICompatConfig::mistral),
        ("TOGETHER_API_KEY", OpenAICompatConfig::together),
        ("FIREWORKS_API_KEY", OpenAICompatConfig::fireworks),
        ("XAI_API_KEY", OpenAICompatConfig::xai),
    ];

    for (env_var, config_fn) in compat_providers {
        if let Ok(key) = std::env::var(env_var)
            && !key.is_empty()
        {
            let config = config_fn(key);
            let name = config.name.clone();
            if !backends.iter().any(|(n, _)| n == &name) {
                info!("Discovered {} provider from {}", name, env_var);
                let compat = OpenAICompatProvider::new(config);
                let adapted: Arc<dyn Provider> = Arc::new(RouterCompatAdapter::new(compat));
                backends.push((name, adapted));
            }
        }
    }

    // 3. Local Ollama provider (auto-detect or via OLLAMA_HOST)
    //
    // We probe Ollama using a raw TCP connect + synchronous HTTP to avoid
    // creating a nested tokio runtime (reqwest::blocking spawns its own
    // runtime, which panics when dropped inside an existing async context).
    {
        let ollama_host = std::env::var("OLLAMA_HOST").unwrap_or_else(|_| "http://localhost:11434".to_string());
        let models_url = format!("{}/v1/models", ollama_host);

        // Parse host:port for a raw TCP probe first
        let probe_addr = ollama_host
            .strip_prefix("http://")
            .or_else(|| ollama_host.strip_prefix("https://"))
            .unwrap_or(&ollama_host);

        let addr_with_port = if probe_addr.contains(':') {
            probe_addr.to_string()
        } else {
            format!("{}:11434", probe_addr)
        };

        let is_reachable = std::net::TcpStream::connect_timeout(
            &addr_with_port.parse().unwrap_or_else(|_| std::net::SocketAddr::from(([127, 0, 0, 1], 11434))),
            std::time::Duration::from_millis(300),
        )
        .is_ok();

        if is_reachable {
            // Ollama is listening — do the model list probe in a blocking
            // thread so reqwest::blocking doesn't panic when its internal
            // runtime is dropped.
            let models_url_clone = models_url.clone();
            let ollama_host_clone = ollama_host.clone();
            let probe_result = std::thread::spawn(move || {
                let client = match reqwest::blocking::Client::builder()
                    .timeout(std::time::Duration::from_millis(1000))
                    .build()
                {
                    Ok(c) => c,
                    Err(_) => return None,
                };
                let resp = client.get(&models_url_clone).send().ok()?;
                if !resp.status().is_success() {
                    return None;
                }
                let body: serde_json::Value = resp.json().ok()?;
                let mut models = Vec::new();
                if let Some(data) = body.get("data").and_then(|d| d.as_array()) {
                    for m in data {
                        if let Some(id) = m.get("id").and_then(|v| v.as_str()) {
                            models.push(clanker_router::Model {
                                id: id.to_string(),
                                name: id.to_string(),
                                provider: "ollama".to_string(),
                                max_input_tokens: 32_768,
                                max_output_tokens: 8_192,
                                supports_thinking: false,
                                supports_images: false,
                                supports_tools: true,
                                input_cost_per_mtok: None,
                                output_cost_per_mtok: None,
                            });
                        }
                    }
                }
                if models.is_empty() {
                    None
                } else {
                    Some((models, ollama_host_clone))
                }
            })
            .join()
            .ok()
            .flatten();

            if let Some((models, host)) = probe_result {
                info!("Discovered Ollama at {} with {} model(s)", host, models.len());
                let config = OpenAICompatConfig::local(format!("{}/v1", host), models);
                let compat = OpenAICompatProvider::new(config);
                let adapted: Arc<dyn Provider> = Arc::new(RouterCompatAdapter::new(compat));
                backends.push(("ollama".to_string(), adapted));
            }
        }
    }

    if backends.is_empty() {
        info!("No API credentials found — starting with unconfigured provider");
        let provider: Arc<dyn Provider> = Arc::new(UnconfiguredProvider);
        backends.push(("unconfigured".to_string(), provider));
    }

    info!(
        "Router initialized with {} provider(s): {}",
        backends.len(),
        backends.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
    );

    // Open the response cache database.
    // Path: ~/.clankers/agent/cache.db (alongside other global config).
    // Skip when CLANKERS_NO_DAEMON is set — test harnesses set this env var,
    // and opening the same redb file from 20+ parallel test processes causes
    // file-lock contention.
    let cache_db = if std::env::var("CLANKERS_NO_DAEMON").is_err() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let cache_db_path = std::path::PathBuf::from(home)
            .join(".clankers")
            .join("agent")
            .join("cache.db");

        match clanker_router::RouterDb::open(&cache_db_path) {
            Ok(db) => {
                info!("Response cache enabled at {}", cache_db_path.display());
                Some(db)
            }
            Err(e) => {
                tracing::warn!("Failed to open cache database: {e} — caching disabled");
                None
            }
        }
    } else {
        None
    };

    // Wire up default fallback chains (Anthropic ↔ OpenAI ↔ DeepSeek)
    let fallbacks = clanker_router::router::FallbackConfig::with_defaults();

    match cache_db {
        Some(db) => Ok(Arc::new(RouterProvider::with_db(backends, db).with_fallbacks(fallbacks))),
        None => Ok(Arc::new(RouterProvider::new(backends).with_fallbacks(fallbacks))),
    }
}

// ── Unconfigured provider (no API keys) ─────────────────────────────────

/// Placeholder provider returned when no API credentials are found.
///
/// Allows the daemon (and other startup paths) to proceed without
/// credentials. Any attempt to actually make an LLM call returns a
/// clear error directing the user to configure authentication.
struct UnconfiguredProvider;

#[async_trait::async_trait]
impl Provider for UnconfiguredProvider {
    async fn complete(
        &self,
        _request: CompletionRequest,
        _tx: tokio::sync::mpsc::Sender<StreamEvent>,
    ) -> Result<()> {
        Err(crate::error::auth_err(
            "No API credentials configured. Run 'clankers auth login', set ANTHROPIC_API_KEY, or start Ollama at localhost:11434.",
        ))
    }

    fn models(&self) -> &[Model] {
        &[]
    }

    fn name(&self) -> &str {
        "unconfigured"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthStoreExt;
    use crate::auth::OAuthCredentials;

    fn codex_store(path: &std::path::Path) {
        let mut store = crate::auth::AuthStore::default();
        store.set_provider_credentials(
            crate::openai_codex::OPENAI_CODEX_PROVIDER,
            "work",
            OAuthCredentials {
                access: "header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9hY2NvdW50X2lkIjoiacctMTIzIn19.signature".to_string(),
                refresh: "refresh".to_string(),
                expires: chrono::Utc::now().timestamp_millis() + 3_600_000,
            },
        );
        store.switch_provider_account(crate::openai_codex::OPENAI_CODEX_PROVIDER, "work");
        store.save(path).expect("auth store should save");
    }

    #[test]
    fn build_router_hides_codex_models_without_credentials() {
        let dir = tempfile::TempDir::new().expect("tempdir should exist");
        let auth_path = dir.path().join("auth.json");
        crate::auth::AuthStore::default().save(&auth_path).expect("auth store should save");

        let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
        let provider = runtime
            .block_on(async { build_router(None, None, &auth_path, None, None) })
            .expect("router should build");
        assert!(
            provider
                .models()
                .iter()
                .all(|model| model.provider != crate::openai_codex::OPENAI_CODEX_PROVIDER)
        );
    }

    #[test]
    fn build_router_discovers_exact_codex_catalog_when_entitled() {
        crate::openai_codex::with_test_probe_hook(
            |_| crate::openai_codex::ProbeOutcome::Entitled,
            || {
                let dir = tempfile::TempDir::new().expect("tempdir should exist");
                let auth_path = dir.path().join("auth.json");
                codex_store(&auth_path);

                let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
                let provider = runtime
                    .block_on(async { build_router(None, None, &auth_path, None, None) })
                    .expect("router should build");
                let ids: Vec<String> = provider
                    .models()
                    .iter()
                    .filter(|model| model.provider == crate::openai_codex::OPENAI_CODEX_PROVIDER)
                    .map(|model| model.id.clone())
                    .collect();
                assert_eq!(
                    ids,
                    crate::openai_codex::OPENAI_CODEX_MODEL_IDS
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                );
            },
        );
    }

    #[test]
    fn build_router_discovers_exact_codex_catalog_from_fallback_auth_store() {
        crate::openai_codex::with_test_probe_hook(
            |_| crate::openai_codex::ProbeOutcome::Entitled,
            || {
                let dir = tempfile::TempDir::new().expect("tempdir should exist");
                let auth_path = dir.path().join("auth.json");
                let fallback_auth_path = dir.path().join("pi-auth.json");
                crate::auth::AuthStore::default().save(&auth_path).expect("auth store should save");
                codex_store(&fallback_auth_path);

                let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
                let provider = runtime
                    .block_on(async { build_router(None, None, &auth_path, Some(&fallback_auth_path), None) })
                    .expect("router should build");
                let ids: Vec<String> = provider
                    .models()
                    .iter()
                    .filter(|model| model.provider == crate::openai_codex::OPENAI_CODEX_PROVIDER)
                    .map(|model| model.id.clone())
                    .collect();
                assert_eq!(
                    ids,
                    crate::openai_codex::OPENAI_CODEX_MODEL_IDS
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                );
            },
        );
    }

    #[test]
    fn build_router_suppresses_codex_catalog_when_not_entitled() {
        crate::openai_codex::with_test_probe_hook(
            |_| crate::openai_codex::ProbeOutcome::NotEntitled(
                "authenticated but not entitled for Codex use".to_string(),
            ),
            || {
                let dir = tempfile::TempDir::new().expect("tempdir should exist");
                let auth_path = dir.path().join("auth.json");
                codex_store(&auth_path);

                let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
                let provider = runtime
                    .block_on(async { build_router(None, None, &auth_path, None, None) })
                    .expect("router should build");
                assert!(
                    provider
                        .models()
                        .iter()
                        .all(|model| model.provider != crate::openai_codex::OPENAI_CODEX_PROVIDER)
                );
            },
        );
    }

    #[test]
    fn build_router_with_rpc_discovers_codex_catalog_from_fallback_auth_store() {
        crate::openai_codex::with_test_probe_hook(
            |_| crate::openai_codex::ProbeOutcome::Entitled,
            || {
                let dir = tempfile::TempDir::new().expect("tempdir should exist");
                let auth_path = dir.path().join("auth.json");
                let fallback_auth_path = dir.path().join("pi-auth.json");
                crate::auth::AuthStore::default().save(&auth_path).expect("auth store should save");
                codex_store(&fallback_auth_path);

                let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
                let provider = runtime
                    .block_on(build_router_with_rpc(None, None, &auth_path, Some(&fallback_auth_path), None))
                    .expect("router should build");
                let ids: Vec<String> = provider
                    .models()
                    .iter()
                    .filter(|model| model.provider == crate::openai_codex::OPENAI_CODEX_PROVIDER)
                    .map(|model| model.id.clone())
                    .collect();
                assert_eq!(
                    ids,
                    crate::openai_codex::OPENAI_CODEX_MODEL_IDS
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                );
            },
        );
    }

    #[test]
    fn build_router_with_rpc_prefers_local_codex_discovery() {
        crate::openai_codex::with_test_probe_hook(
            |_| crate::openai_codex::ProbeOutcome::Entitled,
            || {
                let dir = tempfile::TempDir::new().expect("tempdir should exist");
                let auth_path = dir.path().join("auth.json");
                codex_store(&auth_path);

                let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
                let provider = runtime
                    .block_on(build_router_with_rpc(None, None, &auth_path, None, None))
                    .expect("router should build");
                let ids: Vec<String> = provider
                    .models()
                    .iter()
                    .filter(|model| model.provider == crate::openai_codex::OPENAI_CODEX_PROVIDER)
                    .map(|model| model.id.clone())
                    .collect();
                assert_eq!(
                    ids,
                    crate::openai_codex::OPENAI_CODEX_MODEL_IDS
                        .iter()
                        .map(|id| id.to_string())
                        .collect::<Vec<_>>()
                );
            },
        );
    }
}
