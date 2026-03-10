//! Multi-provider router discovery and construction.
//!
//! Builds a [`RouterProvider`](super::router::RouterProvider) by probing for
//! available API credentials (env vars, auth store, Ollama) and optionally
//! connecting to a running `clankers-router` daemon over iroh RPC.

use std::sync::Arc;

use tracing::info;

use super::Provider;
use super::anthropic::AnthropicProvider;
use super::auth;
use super::credential_manager::CredentialManager;
use crate::error::Result;

/// Build a multi-provider router that auto-discovers available providers.
///
/// Resolution order:
/// 1. Try connecting to a running clankers-router daemon (iroh RPC)
/// 2. Try auto-starting the daemon if `clankers-router` is in PATH
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

    // Skip RPC if CLANKERS_NO_DAEMON is set (useful for testing/debugging)
    if std::env::var("CLANKERS_NO_DAEMON").is_err()
        && let Some(provider) = RpcProvider::auto_start_and_connect().await
    {
        return Ok(provider);
    }

    // Fall back to in-process
    build_router(api_key_override, base_url, auth_store_path, fallback_auth_path, account)
}

/// Build in-process (no RPC). Used as fallback when daemon is unavailable.
pub fn build_router(
    api_key_override: Option<&str>,
    base_url: Option<String>,
    auth_store_path: &std::path::Path,
    fallback_auth_path: Option<&std::path::Path>,
    account: Option<&str>,
) -> Result<Arc<dyn Provider>> {
    use clankers_router::backends::openai_compat::OpenAICompatConfig;
    use clankers_router::backends::openai_compat::OpenAICompatProvider;

    use super::router::RouterCompatAdapter;
    use super::router::RouterProvider;

    let mut backends: Vec<(String, Arc<dyn Provider>)> = Vec::new();

    // 1. Anthropic (OAuth + API key + env var)
    let anthropic_cred =
        auth::resolve_credential_with_fallback(api_key_override, auth_store_path, fallback_auth_path, account);

    if let Some(credential) = anthropic_cred {
        let provider: Arc<dyn Provider> = if credential.is_oauth() {
            let cm = CredentialManager::new(
                credential,
                auth_store_path.to_path_buf(),
                fallback_auth_path.map(|p| p.to_path_buf()),
            );
            Arc::new(AnthropicProvider::with_credential_manager(cm, base_url))
        } else {
            Arc::new(AnthropicProvider::new(credential, base_url))
        };
        backends.push(("anthropic".to_string(), provider));
    }

    // 2. OpenAI-compatible providers from env vars
    type CompatFactory = fn(String) -> OpenAICompatConfig;
    let compat_providers: &[(&str, CompatFactory)] = &[
        ("OPENAI_API_KEY", OpenAICompatConfig::openai),
        ("OPENROUTER_API_KEY", OpenAICompatConfig::openrouter),
        ("GROQ_API_KEY", OpenAICompatConfig::groq),
        ("DEEPSEEK_API_KEY", OpenAICompatConfig::deepseek),
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
                            models.push(clankers_router::Model {
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
        return Err(crate::error::auth_err("No API credentials found. Set ANTHROPIC_API_KEY, OPENAI_API_KEY, or run 'clankers auth login'. Ollama also supported at localhost:11434."));
    }

    info!(
        "Router initialized with {} provider(s): {}",
        backends.len(),
        backends.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")
    );

    Ok(Arc::new(RouterProvider::new(backends)))
}
