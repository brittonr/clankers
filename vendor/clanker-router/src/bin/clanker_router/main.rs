//! clanker-router CLI + TUI
//!
//! Standalone binary for managing LLM provider routing, credentials,
//! model discovery, and interactive chat.

mod tui;

use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use clanker_router::Router;
use clanker_router::auth::AuthStore;
use clanker_router::auth::AuthStorePaths;
use clanker_router::auth::ImportTarget;
use clanker_router::auth::OAuthFlow;
use clanker_router::auth::PendingOAuthLogin;
use clanker_router::auth::StoredCredential;
use clanker_router::auth::pending_oauth_login_path;
use clanker_router::auth::env_var_for_provider;
use clanker_router::auth::resolve_credential;
use clanker_router::backends::huggingface::HubClient;
use clanker_router::backends::openai_codex;
use clanker_router::backends::openai_codex::OpenAICodexProvider;
use clanker_router::backends::openai_compat::OpenAICompatConfig;
use clanker_router::backends::openai_compat::OpenAICompatProvider;
use clanker_router::credential::CredentialManager;
use clanker_router::credential_pool::CredentialPool;
use clanker_router::credential_pool::SelectionStrategy;
use clanker_router::model::ModelAliases;
use clanker_router::provider::CompletionRequest;
use clanker_router::provider::Provider;
use clanker_router::streaming::ContentDelta;
use clanker_router::streaming::StreamEvent;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use serde::Deserialize;
use tokio::sync::mpsc;

// ── CLI definition ──────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "clanker-router",
    about = "LLM provider router — route, chat, and manage credentials",
    version
)]
struct Cli {
    /// Model to use
    #[arg(short, long, default_value = "gpt-4o")]
    model: String,

    /// Provider override (anthropic, openai, groq, deepseek, openrouter, local)
    #[arg(short, long)]
    provider: Option<String>,

    /// API key override
    #[arg(long, env = "CLANKERS_ROUTER_API_KEY")]
    api_key: Option<String>,

    /// API base URL override (for local/custom endpoints)
    #[arg(long)]
    api_base: Option<String>,

    /// Auth store path (single-file mode)
    #[arg(long)]
    auth_file: Option<PathBuf>,

    /// Read-only seed auth store path (managed service mode)
    #[arg(long)]
    auth_seed_file: Option<PathBuf>,

    /// Writable runtime auth store path (managed service mode)
    #[arg(long)]
    auth_runtime_file: Option<PathBuf>,

    /// JSON file describing extra OpenAI-compatible providers to register
    #[arg(long)]
    local_provider_config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Send a single prompt and print the response (non-interactive)
    Ask {
        /// The prompt text
        prompt: String,
        /// System prompt
        #[arg(long)]
        system: Option<String>,
        /// Max output tokens
        #[arg(long)]
        max_tokens: Option<usize>,
        /// Temperature (0.0-1.0)
        #[arg(long)]
        temperature: Option<f64>,
        /// Output format
        #[arg(long, value_enum, default_value_t = OutputFormat::Text)]
        format: OutputFormat,
    },
    /// Interactive TUI chat
    Chat {
        /// System prompt
        #[arg(long)]
        system: Option<String>,
    },
    /// List available models
    Models {
        /// Filter by provider
        #[arg(long)]
        provider: Option<String>,
        /// Show detailed info (JSON)
        #[arg(long)]
        json: bool,
    },
    /// Manage credentials
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Resolve a model alias to its full ID and provider
    Resolve {
        /// Model name or alias
        name: String,
    },
    /// Show provider status and health
    Status,
    /// Show token usage and cost statistics
    Usage {
        /// Number of recent days to show (default: 7)
        #[arg(long, default_value_t = 7)]
        days: usize,
        /// Show all-time totals only
        #[arg(long)]
        total: bool,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// HuggingFace Hub — search, pull, and serve models
    Hf {
        #[command(subcommand)]
        action: HfAction,
    },
    /// Run the router as an RPC daemon + OpenAI-compatible proxy
    Serve {
        /// Run in background (detach from terminal)
        #[arg(long)]
        daemon: bool,
        /// HTTP proxy listen address (default: 127.0.0.1:4000)
        #[arg(long, default_value = "127.0.0.1:4000")]
        proxy_addr: String,
        /// API keys allowed to access the proxy (repeatable; omit for no auth)
        #[arg(long)]
        proxy_key: Vec<String>,
    },
}

#[derive(Subcommand)]
enum AuthAction {
    /// Show credential status
    Status {
        /// Provider name (shows all if omitted)
        #[arg(long)]
        provider: Option<String>,
        /// Show all configured providers
        #[arg(long)]
        all: bool,
    },
    /// Authenticate with an OAuth provider
    Login {
        /// Provider name (`anthropic`, `openai-codex`)
        #[arg(long)]
        provider: Option<String>,
        /// Account name
        #[arg(long, default_value = "default")]
        account: String,
        /// Complete login with code from browser (code#state or callback URL)
        #[arg(long)]
        code: Option<String>,
    },
    /// Export one provider/account record as JSON
    Export {
        /// Provider name
        provider: String,
        /// Account name
        #[arg(long, default_value = "default")]
        account: String,
    },
    /// Import one provider/account record from JSON
    Import {
        /// Path to JSON record (`-` for stdin)
        #[arg(long, default_value = "-")]
        input: String,
        /// Write target (`auto`, `file`, `seed`, `runtime`)
        #[arg(long, default_value = "auto")]
        target: String,
    },
    /// Set an API key for a provider
    SetKey {
        /// Provider name
        provider: String,
        /// API key (will prompt on stdin if omitted)
        key: Option<String>,
        /// Account name
        #[arg(long, default_value = "default")]
        account: String,
        /// Label for this key
        #[arg(long)]
        label: Option<String>,
    },
    /// Remove credentials for a provider
    Remove {
        /// Provider name
        provider: String,
        /// Account name
        #[arg(long, default_value = "default")]
        account: String,
    },
    /// Switch active account for a provider
    Switch {
        /// Provider name
        provider: String,
        /// Account name to activate
        account: String,
    },
    /// List accounts for a provider
    Accounts {
        /// Provider name (shows all if omitted)
        provider: Option<String>,
    },
}

#[derive(Subcommand)]
enum HfAction {
    /// Search HuggingFace Hub for text-generation models
    Search {
        /// Search query (model name, author, or keywords)
        query: String,
        /// Max results (default: 20)
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Output as JSON
        #[arg(long)]
        json: bool,
    },
    /// Show details about a specific model on the Hub
    Info {
        /// Model ID (e.g., "meta-llama/Llama-3.3-70B-Instruct")
        model: String,
    },
    /// List available GGUF files for a model
    Files {
        /// Model ID (e.g., "bartowski/Llama-3.3-70B-Instruct-GGUF")
        model: String,
    },
    /// Pull (download) a GGUF model from HuggingFace Hub
    Pull {
        /// Model ID (e.g., "bartowski/Llama-3.3-70B-Instruct-GGUF")
        model: String,
        /// Quantization to download (e.g., Q4_K_M, Q8_0). Defaults to smallest.
        #[arg(short, long)]
        quant: Option<String>,
        /// Register with Ollama after download
        #[arg(long)]
        ollama: bool,
        /// Custom Ollama model name (implies --ollama)
        #[arg(long)]
        ollama_name: Option<String>,
    },
    /// List locally cached (pulled) models
    List,
    /// Remove a cached model
    Remove {
        /// Model ID to remove
        model: String,
    },
    /// Register a previously pulled model with Ollama
    Ollama {
        /// Model ID (must be already pulled)
        model: String,
        /// Custom Ollama name
        #[arg(long)]
        name: Option<String>,
    },
}

#[derive(ValueEnum, Clone, Copy)]
enum OutputFormat {
    /// Plain text (streamed tokens)
    Text,
    /// Full JSON response
    Json,
    /// Markdown
    Markdown,
}

// ── Paths ───────────────────────────────────────────────────────────────

fn default_auth_path() -> PathBuf {
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("clanker-router").join("auth.json")
}

fn resolve_auth_path(cli: &Cli) -> PathBuf {
    cli.auth_file.clone().unwrap_or_else(default_auth_path)
}

fn resolve_auth_paths(cli: &Cli) -> AuthStorePaths {
    if cli.auth_seed_file.is_some() || cli.auth_runtime_file.is_some() {
        AuthStorePaths {
            auth_file: None,
            seed_file: cli.auth_seed_file.clone(),
            runtime_file: cli.auth_runtime_file.clone(),
        }
    } else {
        AuthStorePaths::single(resolve_auth_path(cli))
    }
}

fn auth_base_dir(auth_paths: &AuthStorePaths) -> PathBuf {
    auth_paths
        .pending_oauth_base_dir()
        .unwrap_or_else(|| default_auth_path().parent().map(PathBuf::from).unwrap_or_else(|| PathBuf::from(".")))
}

// ── Provider construction ───────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct LocalProviderConfig {
    name: String,
    api_base: String,
    models: Vec<String>,
}

fn load_local_provider_configs(path: &std::path::Path) -> Result<Vec<LocalProviderConfig>, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse {}: {e}", path.display()))
}

fn local_models(provider_name: &str, model_ids: &[String]) -> Vec<clanker_router::Model> {
    model_ids
        .iter()
        .map(|model_id| clanker_router::Model {
            id: model_id.clone(),
            name: model_id.clone(),
            provider: provider_name.to_string(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: false,
            supports_tools: true,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        })
        .collect()
}

async fn build_providers(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths) -> Vec<Arc<dyn Provider>> {
    let mut providers: Vec<Arc<dyn Provider>> = Vec::new();

    // ── Helpers ──────────────────────────────────────────────────────

    // Collect all credentials for a provider (multi-account pool).
    // Returns: Vec<(account_name, StoredCredential)> with active first.
    // Also checks env var as an additional source.
    let all_creds_for = |name: &str| -> Vec<(String, StoredCredential)> {
        let mut creds = auth_store.all_credentials(name);

        // CLI override replaces the pool entirely
        if cli.provider.as_deref() == Some(name) {
            if let Some(ref k) = cli.api_key {
                return vec![("cli-override".into(), StoredCredential::ApiKey {
                    api_key: k.clone(),
                    label: Some("cli-override".into()),
                })];
            }
        }

        // Use env var ONLY as a fallback when the auth store has no
        // credentials for this provider. This prevents dummy/proxy API
        // keys (e.g. ANTHROPIC_API_KEY=sk-ant-proxy set by a co-hosted
        // daemon) from contaminating the credential pool alongside real
        // OAuth tokens from the auth store.
        if creds.is_empty() {
            if let Some(env_var) = env_var_for_provider(name)
                && let Ok(key) = std::env::var(env_var)
                && !key.is_empty()
            {
                let env_label = format!("env:{}", env_var);
                creds.push((env_label.clone(), StoredCredential::ApiKey {
                    api_key: key,
                    label: Some(env_label),
                }));
            }
        }

        creds
    };

    // Helper: build an OpenAI-compat provider with optional pool
    let build_openai_compat = |name: &str, config_fn: fn(String) -> OpenAICompatConfig| -> Option<Arc<dyn Provider>> {
        let creds = all_creds_for(name);
        if creds.is_empty() {
            return None;
        }
        let primary_key = creds[0].1.token().to_string();
        let config = config_fn(primary_key);

        if creds.len() > 1 {
            let pool = CredentialPool::new(creds, SelectionStrategy::Failover);
            tracing::info!("{}: {} account(s) in pool", name, pool.len());
            Some(OpenAICompatProvider::with_pool(config, pool))
        } else {
            Some(OpenAICompatProvider::new(config))
        }
    };

    // ── Anthropic (native Messages API) ─────────────────────────────

    {
        let creds = all_creds_for("anthropic");
        if !creds.is_empty() {
            use clanker_router::backends::anthropic::AnthropicProvider;
            use clanker_router::backends::anthropic::Credential;
            let base_url = if cli.provider.as_deref() == Some("anthropic") {
                cli.api_base.clone()
            } else {
                None
            };

            if creds.len() > 1 {
                // Multi-account: build a credential pool
                let pool = CredentialPool::new(creds, SelectionStrategy::Failover);
                tracing::info!("anthropic: {} account(s) in pool", pool.len());
                providers.push(AnthropicProvider::with_pool_managed(pool, base_url, auth_paths.clone()));
            } else {
                // Single account: legacy path
                let cred = &creds[0].1;
                let anthropic_cred = if cred.is_oauth() || clanker_router::auth::is_oauth_token(cred.token()) {
                    Credential::OAuth(cred.token().to_string())
                } else {
                    Credential::ApiKey(cred.token().to_string())
                };
                providers.push(AnthropicProvider::new_managed(anthropic_cred, base_url, auth_paths.clone()));
            }
        }
    }

    // ── OpenAI Codex ────────────────────────────────────────────────

    {
        if let Some(credential) = auth_store.active_credential(openai_codex::OPENAI_CODEX_PROVIDER).cloned() {
            let account = auth_store
                .providers
                .get(openai_codex::OPENAI_CODEX_PROVIDER)
                .and_then(|provider| provider.active_account.clone())
                .unwrap_or_else(|| "default".to_string());
            let manager = CredentialManager::with_refresh_fn(
                openai_codex::OPENAI_CODEX_PROVIDER.to_string(),
                credential,
                auth_paths.clone(),
                None,
                openai_codex::refresh_fn_for_codex(),
            );
            let models = openai_codex::catalog_for_active_account_with_manager(
                auth_store,
                &account,
                Some(manager.as_ref()),
            )
            .await;
            if !models.is_empty() {
                providers.push(OpenAICodexProvider::new(manager, models, account));
            }
        }
    }

    // ── OpenAI ──────────────────────────────────────────────────────

    {
        let creds = all_creds_for("openai");
        if !creds.is_empty() {
            let primary_key = creds[0].1.token().to_string();
            let mut config = OpenAICompatConfig::openai(primary_key);
            if cli.provider.as_deref() == Some("openai") {
                if let Some(ref base) = cli.api_base {
                    config.base_url = base.clone();
                }
            }
            if creds.len() > 1 {
                let pool = CredentialPool::new(creds, SelectionStrategy::Failover);
                tracing::info!("openai: {} account(s) in pool", pool.len());
                providers.push(OpenAICompatProvider::with_pool(config, pool));
            } else {
                providers.push(OpenAICompatProvider::new(config));
            }
        }
    }

    // ── Simple OpenAI-compat providers (single-function setup) ──────

    if let Some(p) = build_openai_compat("groq", OpenAICompatConfig::groq) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("deepseek", OpenAICompatConfig::deepseek) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("openrouter", OpenAICompatConfig::openrouter) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("mistral", OpenAICompatConfig::mistral) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("together", OpenAICompatConfig::together) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("fireworks", OpenAICompatConfig::fireworks) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("perplexity", OpenAICompatConfig::perplexity) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("xai", OpenAICompatConfig::xai) {
        providers.push(p);
    }
    if let Some(p) = build_openai_compat("huggingface", OpenAICompatConfig::huggingface) {
        providers.push(p);
    }

    // ── Google/Gemini (check both provider names) ───────────────────

    {
        let mut creds = all_creds_for("google");
        if creds.is_empty() {
            creds = all_creds_for("gemini");
        }
        if !creds.is_empty() {
            let primary_key = creds[0].1.token().to_string();
            let config = OpenAICompatConfig::google(primary_key);
            if creds.len() > 1 {
                let pool = CredentialPool::new(creds, SelectionStrategy::Failover);
                providers.push(OpenAICompatProvider::with_pool(config, pool));
            } else {
                providers.push(OpenAICompatProvider::new(config));
            }
        }
    }

    // ── Local (always available if --api-base points to local) ──────

    if cli.provider.as_deref() == Some("local") {
        let base = cli.api_base.clone().unwrap_or_else(|| "http://localhost:11434/v1".into());
        let models = vec![clanker_router::Model {
            id: cli.model.clone(),
            name: cli.model.clone(),
            provider: "local".into(),
            max_input_tokens: 128_000,
            max_output_tokens: 16_384,
            supports_thinking: false,
            supports_images: false,
            supports_tools: false,
            input_cost_per_mtok: None,
            output_cost_per_mtok: None,
        }];
        providers.push(OpenAICompatProvider::new(OpenAICompatConfig::local(base, models)));
    }

    if let Some(ref path) = cli.local_provider_config {
        match load_local_provider_configs(path) {
            Ok(configs) => {
                for config in configs {
                    if config.models.is_empty() {
                        tracing::warn!("skipping local provider '{}' with no models", config.name);
                        continue;
                    }

                    providers.push(OpenAICompatProvider::new(OpenAICompatConfig {
                        name: config.name.clone(),
                        base_url: config.api_base,
                        api_key: String::new(),
                        extra_headers: vec![],
                        models: local_models(&config.name, &config.models),
                        timeout: std::time::Duration::from_secs(600),
                    }));
                }
            }
            Err(err) => {
                tracing::warn!("failed to load local provider config: {err}");
            }
        }
    }

    providers
}

fn default_db_path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("clanker-router").join("router.db")
}

async fn build_router(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths) -> Router {
    let db_path = default_db_path();
    let mut router = match clanker_router::RouterDb::open(&db_path) {
        Ok(db) => {
            tracing::debug!("Opened router database at {}", db_path.display());
            Router::with_db(&cli.model, db)
        }
        Err(e) => {
            tracing::warn!("Failed to open router database ({}), running without persistence", e);
            Router::new(&cli.model)
        }
    };
    for provider in build_providers(cli, auth_store, auth_paths).await {
        router.register_provider(provider);
    }

    // Enable default fallback chains so rate-limited providers
    // automatically fall over to alternatives.
    router.set_fallbacks(clanker_router::FallbackConfig::with_defaults());

    router
}

// ── Main ────────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    // Logging — silence iroh and its transitive dependencies
    let iroh_filters = ",iroh=error,iroh_base=error,iroh_blobs=error,iroh_io=error,\
        iroh_metrics=error,iroh_quinn=error,iroh_quinn_proto=error,iroh_quinn_udp=error,\
        iroh_relay=error,iroh_tickets=error,netwatch=error,portmapper=error,swarm_discovery=error";
    if cli.verbose {
        tracing_subscriber::fmt()
            .with_env_filter(format!("clanker_router=debug{iroh_filters}"))
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(format!("clanker_router=warn{iroh_filters}"))
            .with_writer(std::io::stderr)
            .init();
    }

    if cli.auth_file.is_some() && (cli.auth_seed_file.is_some() || cli.auth_runtime_file.is_some()) {
        eprintln!("Use either --auth-file or --auth-seed-file/--auth-runtime-file, not both.");
        std::process::exit(1);
    }

    let auth_paths = resolve_auth_paths(&cli);
    let auth_store = auth_paths.load_effective().into_store();

    match &cli.command {
        None | Some(Commands::Chat { .. }) => {
            let system = match &cli.command {
                Some(Commands::Chat { system }) => system.clone(),
                _ => None,
            };
            run_tui(&cli, &auth_store, &auth_paths, system).await;
        }
        Some(Commands::Ask {
            prompt,
            system,
            max_tokens,
            temperature,
            format,
        }) => {
            run_ask(&cli, &auth_store, &auth_paths, prompt, system.clone(), *max_tokens, *temperature, format).await;
        }
        Some(Commands::Models { provider, json }) => {
            run_models(&cli, &auth_store, &auth_paths, provider.as_deref(), *json).await;
        }
        Some(Commands::Auth { action }) => {
            run_auth(&auth_paths, action).await;
        }
        Some(Commands::Resolve { name }) => {
            run_resolve(&cli, &auth_store, &auth_paths, name).await;
        }
        Some(Commands::Status) => {
            run_status(&cli, &auth_store, &auth_paths).await;
        }
        Some(Commands::Usage { days, total, json }) => {
            run_usage(*days, *total, *json);
        }
        Some(Commands::Hf { action }) => {
            run_hf(&cli, &auth_store, &auth_paths, action).await;
        }
        Some(Commands::Serve {
            daemon,
            proxy_addr,
            proxy_key,
        }) => {
            run_serve(&cli, &auth_store, &auth_paths, *daemon, proxy_addr, proxy_key).await;
        }
    }
}

// ── Ask (non-interactive) ───────────────────────────────────────────────

fn requests_openai_codex_explicitly(cli: &Cli) -> bool {
    cli.provider.as_deref() == Some(openai_codex::OPENAI_CODEX_PROVIDER)
        || cli.model.trim().starts_with("openai-codex/")
}

fn exit_no_providers(cli: &Cli) -> ! {
    if requests_openai_codex_explicitly(cli) {
        eprintln!(
            "Error: openai-codex is unavailable because no valid openai-codex credential is active."
        );
        eprintln!("Import or log in to an openai-codex account first:");
        eprintln!("  clanker-router auth import --input record.json --target file");
        eprintln!("  clanker-router auth login --provider openai-codex");
    } else {
        eprintln!("Error: No providers configured. Set an API key:");
        eprintln!("  clanker-router auth set-key openai sk-...");
        eprintln!("  export OPENAI_API_KEY=sk-...");
    }
    std::process::exit(1);
}

async fn run_ask(
    cli: &Cli,
    auth_store: &AuthStore,
    auth_paths: &AuthStorePaths,
    prompt: &str,
    system: Option<String>,
    max_tokens: Option<usize>,
    temperature: Option<f64>,
    format: &OutputFormat,
) {
    let router = build_router(cli, auth_store, auth_paths).await;

    if router.provider_names().is_empty() {
        exit_no_providers(cli);
    }

    let request = CompletionRequest {
        model: cli.model.clone(),
        messages: vec![serde_json::json!({"role": "user", "content": prompt})],
        system_prompt: system,
        max_tokens,
        temperature,
        tools: vec![],
        thinking: None,
        no_cache: false,
        cache_ttl: None,
        extra_params: Default::default(),
    };

    let (tx, mut rx) = mpsc::channel(64);

    let complete_handle = tokio::spawn(async move { router.complete(request, tx).await });

    let mut full_text = String::new();
    let mut usage = clanker_router::provider::Usage::default();

    while let Some(event) = rx.recv().await {
        match event {
            StreamEvent::ContentBlockDelta {
                delta: ContentDelta::TextDelta { text },
                ..
            } => {
                match *format {
                    OutputFormat::Text | OutputFormat::Markdown => {
                        print!("{}", text);
                        use std::io::Write;
                        let _ = std::io::stdout().flush();
                    }
                    OutputFormat::Json => {}
                }
                full_text.push_str(&text);
            }
            StreamEvent::MessageDelta { usage: u, .. } => {
                usage = u;
            }
            StreamEvent::Error { error } => {
                eprintln!("\nError: {}", error);
                std::process::exit(1);
            }
            _ => {}
        }
    }

    match *format {
        OutputFormat::Text | OutputFormat::Markdown => {
            println!();
            if cli.verbose {
                eprintln!("--- {} input, {} output tokens ---", usage.input_tokens, usage.output_tokens);
            }
        }
        OutputFormat::Json => {
            let output = serde_json::json!({
                "model": cli.model,
                "content": full_text,
                "usage": {
                    "input_tokens": usage.input_tokens,
                    "output_tokens": usage.output_tokens,
                }
            });
            println!("{}", serde_json::to_string_pretty(&output).unwrap());
        }
    }

    if let Err(e) = complete_handle.await.unwrap() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}

// ── Models ──────────────────────────────────────────────────────────────

async fn run_models(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths, provider_filter: Option<&str>, json: bool) {
    let router = build_router(cli, auth_store, auth_paths).await;
    let models = if let Some(prov) = provider_filter {
        router.registry().list_for_provider(prov).into_iter().cloned().collect::<Vec<_>>()
    } else {
        router.list_models().into_iter().cloned().collect::<Vec<_>>()
    };

    if models.is_empty() {
        eprintln!("No models available. Configure a provider first:");
        eprintln!("  clanker-router auth set-key openai sk-...");
        return;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&models).unwrap());
    } else {
        println!(
            "{:<40} {:<12} {:>8} {:>8} {:>6} {:>6} {:>6}",
            "MODEL", "PROVIDER", "IN_CTX", "OUT_MAX", "THINK", "IMAGE", "TOOLS"
        );
        println!("{}", "─".repeat(92));
        for m in &models {
            println!(
                "{:<40} {:<12} {:>7}K {:>7}K {:>6} {:>6} {:>6}",
                m.id,
                m.provider,
                m.max_input_tokens / 1000,
                m.max_output_tokens / 1000,
                if m.supports_thinking { "✓" } else { "·" },
                if m.supports_images { "✓" } else { "·" },
                if m.supports_tools { "✓" } else { "·" },
            );
        }
        println!("\n{} model(s)", models.len());
    }
}

// ── Auth ────────────────────────────────────────────────────────────────

fn format_expires_in(expires_at_ms: i64) -> String {
    let remaining_ms = expires_at_ms - chrono::Utc::now().timestamp_millis();
    if remaining_ms <= 0 {
        return "expired".to_string();
    }
    let mins = remaining_ms / 60_000;
    if mins > 60 {
        format!("{}h {}m", mins / 60, mins % 60)
    } else {
        format!("{}m", mins)
    }
}

fn describe_credential(cred: &StoredCredential) -> String {
    match cred {
        StoredCredential::ApiKey { .. } => "api key".to_string(),
        StoredCredential::OAuth { expires_at_ms, .. } => {
            if cred.is_expired() {
                "oauth expired".to_string()
            } else {
                format!("oauth valid (expires in {})", format_expires_in(*expires_at_ms))
            }
        }
    }
}

async fn provider_status_lines(
    effective: &clanker_router::auth::EffectiveAuthStore,
    auth_paths: &AuthStorePaths,
    provider: &str,
) -> Vec<String> {
    let mut infos = effective.list_accounts_with_sources(provider);
    infos.sort_by(|a, b| b.info.is_active.cmp(&a.info.is_active).then_with(|| a.info.name.cmp(&b.info.name)));

    let mut lines = Vec::new();
    for sourced in infos {
        let marker = if sourced.info.is_active { "▸" } else { " " };
        let label = sourced
            .info
            .label
            .as_ref()
            .map(|value| format!(" ({value})"))
            .unwrap_or_default();
        let base = effective
            .store()
            .credential_for(provider, &sourced.info.name)
            .map(describe_credential)
            .unwrap_or_else(|| "unknown".to_string());
        let detail = if provider == openai_codex::OPENAI_CODEX_PROVIDER {
            let manager = effective
                .store()
                .credential_for(provider, &sourced.info.name)
                .cloned()
                .map(|credential| {
                    CredentialManager::with_refresh_fn(
                        provider.to_string(),
                        credential,
                        auth_paths.clone(),
                        None,
                        openai_codex::refresh_fn_for_codex(),
                    )
                });
            match openai_codex::codex_status_suffix_with_manager(
                effective.store(),
                &sourced.info.name,
                manager.as_deref(),
            )
            .await
            {
                Some(suffix) => format!("{}; {}", base, suffix),
                None => base,
            }
        } else {
            base
        };
        lines.push(format!(
            "  {} {}{} [{}; source={}] — {}",
            marker,
            sourced.info.name,
            label,
            if sourced.info.is_oauth { "oauth" } else { "api-key" },
            sourced.source.label(),
            detail,
        ));
    }
    lines
}

fn parse_import_target(target: &str) -> ImportTarget {
    match target {
        "auto" => ImportTarget::Auto,
        "file" => ImportTarget::File,
        "seed" => ImportTarget::Seed,
        "runtime" => ImportTarget::Runtime,
        other => {
            eprintln!("Unknown import target '{}'. Use one of: auto, file, seed, runtime", other);
            std::process::exit(1);
        }
    }
}

async fn run_auth(auth_paths: &AuthStorePaths, action: &AuthAction) {
    match action {
        AuthAction::Login {
            provider,
            account,
            code,
        } => {
            run_auth_login(auth_paths, provider.as_deref(), account, code.as_deref()).await;
        }
        AuthAction::Status { provider, all } => {
            let effective = auth_paths.load_effective();
            let store = effective.store();
            let providers: Vec<String> = if *all || provider.is_none() {
                store.configured_providers().into_iter().map(ToString::to_string).collect()
            } else {
                vec![provider.clone().expect("provider checked above")]
            };

            if providers.is_empty() {
                println!("No credentials configured.");
                println!("\nEnvironment variables:");
                for p in [
                    "anthropic",
                    "openai",
                    "openrouter",
                    "huggingface",
                    "groq",
                    "deepseek",
                    "mistral",
                    "together",
                    "fireworks",
                    "perplexity",
                    "cohere",
                    "xai",
                ] {
                    if let Some(var) = env_var_for_provider(p) {
                        let status = if std::env::var(var).is_ok() { "✓ set" } else { "· not set" };
                        println!("  {} {} ({})", status, p, var);
                    }
                }
            } else {
                for (idx, provider_name) in providers.iter().enumerate() {
                    if idx > 0 {
                        println!();
                    }
                    println!("{}:", provider_name);
                    for line in provider_status_lines(&effective, auth_paths, provider_name).await {
                        println!("{}", line);
                    }
                }
            }

            if auth_paths.is_layered() {
                if let Some(seed) = &auth_paths.seed_file {
                    println!("\nSeed auth file: {}", seed.display());
                }
                if let Some(runtime) = &auth_paths.runtime_file {
                    println!("Runtime auth file: {}", runtime.display());
                }
            } else if let Some(path) = auth_paths.write_path() {
                println!("\nAuth file: {}", path.display());
            }
        }
        AuthAction::Export { provider, account } => {
            let effective = auth_paths.load_effective();
            let Some(record) = effective.export_account(provider, account) else {
                eprintln!("Account not found: {}/{}", provider, account);
                std::process::exit(1);
            };
            println!("{}", serde_json::to_string_pretty(&record).expect("export should serialize"));
        }
        AuthAction::Import { input, target } => {
            let raw = if input == "-" {
                let mut buf = String::new();
                std::io::stdin().read_to_string(&mut buf).expect("failed to read stdin");
                buf
            } else {
                std::fs::read_to_string(input).expect("failed to read import file")
            };
            let record: clanker_router::auth::ProviderAccountExport =
                serde_json::from_str(&raw).expect("failed to parse auth import record");
            auth_paths
                .import_account(&record, parse_import_target(target))
                .expect("failed to import auth record");
            println!("Imported {}/{} into {} auth store", record.provider, record.account, target);
        }
        AuthAction::SetKey {
            provider,
            key,
            account,
            label,
        } => {
            let api_key = if let Some(k) = key {
                k.clone()
            } else {
                eprint!("Enter API key for {}: ", provider);
                let mut buf = String::new();
                std::io::stdin().read_line(&mut buf).expect("failed to read");
                buf.trim().to_string()
            };

            if api_key.is_empty() {
                eprintln!("Error: empty API key");
                std::process::exit(1);
            }

            let credential = if clanker_router::auth::is_oauth_token(&api_key) {
                let one_year_ms = chrono::Utc::now().timestamp_millis() + 365 * 24 * 3600 * 1000;
                println!("Detected OAuth access token — storing as OAuth credential");
                StoredCredential::OAuth {
                    access_token: api_key,
                    refresh_token: String::new(),
                    expires_at_ms: one_year_ms,
                    label: label.clone(),
                }
            } else {
                StoredCredential::ApiKey {
                    api_key,
                    label: label.clone(),
                }
            };

            auth_paths
                .mutate_write_store(|store| {
                    store.set_credential(provider, account, credential.clone());
                    store.switch_account(provider, account);
                })
                .expect("failed to save auth store");
            println!(
                "Saved {} key for account '{}' in {}",
                provider,
                account,
                auth_paths.write_path().expect("write path should exist").display()
            );
        }
        AuthAction::Remove { provider, account } => {
            let effective = auth_paths.load_effective();
            let source = effective.source_for(provider, account);
            if auth_paths.is_layered() && matches!(source, Some(clanker_router::auth::AuthRecordSource::Seed)) {
                eprintln!("Cannot remove seeded account {}/{} without editing the seed auth store.", provider, account);
                std::process::exit(1);
            }

            let mut store = auth_paths.load_write_store();
            if store.remove_account(provider, account) {
                auth_paths.save_write_store(&store).expect("failed to save");
                println!("Removed {}/{}", provider, account);
            } else {
                eprintln!("Account not found in writable auth store: {}/{}", provider, account);
                std::process::exit(1);
            }
        }
        AuthAction::Switch { provider, account } => {
            let effective = auth_paths.load_effective();
            if effective.store().credential_for(provider, account).is_none() {
                eprintln!("Account not found: {}/{}", provider, account);
                std::process::exit(1);
            }
            auth_paths
                .mutate_write_store(|store| {
                    let provider_auth = store.providers.entry(provider.clone()).or_default();
                    provider_auth.active_account = Some(account.clone());
                })
                .expect("failed to save auth store");
            println!("Switched {} to account '{}'", provider, account);
        }
        AuthAction::Accounts { provider } => {
            let effective = auth_paths.load_effective();
            let providers: Vec<String> = if let Some(provider) = provider {
                vec![provider.clone()]
            } else {
                effective.store().configured_providers().into_iter().map(ToString::to_string).collect()
            };

            if providers.is_empty() {
                println!("No accounts configured.");
                return;
            }

            for (idx, provider_name) in providers.iter().enumerate() {
                if idx > 0 {
                    println!();
                }
                println!("{}:", provider_name);
                for line in provider_status_lines(&effective, auth_paths, provider_name).await {
                    println!("{}", line);
                }
            }
        }
    }
}

// ── OAuth login ─────────────────────────────────────────────────────────

async fn run_auth_login(
    auth_paths: &AuthStorePaths,
    provider: Option<&str>,
    account: &str,
    code_input: Option<&str>,
) {
    let oauth_flow = OAuthFlow::from_provider(provider).expect("provider should be supported");
    let provider_name = oauth_flow.provider_name();
    let base_dir = auth_base_dir(auth_paths);
    let verifier_path = pending_oauth_login_path(&base_dir, provider_name, account);
    let legacy_path = clanker_router::auth::legacy_pending_oauth_login_path(&base_dir);

    let input = if let Some(input) = code_input {
        input.to_string()
    } else {
        let (url, verifier) = oauth_flow.build_auth_url().expect("oauth url should build");
        let pending = PendingOAuthLogin::new(provider_name, account.to_string(), verifier);
        pending.save(&verifier_path).expect("failed to persist pending login");

        println!("Logging in to provider '{}' as account '{}'.", provider_name, account);
        if open::that_detached(&url).is_ok() {
            println!("Opening browser automatically...\n");
        } else {
            println!("Could not open browser automatically.\n");
        }
        println!("Ctrl+Click or open this URL in your browser:\n\n  \x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\n", url, url);
        println!(
            "After authorizing, paste the code or callback URL.\n\
             Accepted formats:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE\n"
        );

        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).expect("failed to read input");
        buf.trim().to_string()
    };

    let (code, state) = parse_oauth_callback(&input).unwrap_or_else(|msg| {
        eprintln!("Error: {}", msg);
        std::process::exit(1);
    });

    let pending = PendingOAuthLogin::load(&verifier_path).or_else(|| PendingOAuthLogin::load(&legacy_path)).unwrap_or_else(|| {
        eprintln!("Error: No login in progress. Run `clanker-router auth login` first.");
        std::process::exit(1);
    });

    let creds = oauth_flow
        .exchange_code(&code, &state, &pending.verifier)
        .await
        .unwrap_or_else(|e| {
            eprintln!("Login failed: {}", e);
            std::process::exit(1);
        });

    std::fs::remove_file(&verifier_path).ok();
    std::fs::remove_file(&legacy_path).ok();

    auth_paths
        .mutate_write_store(|store| {
            store.set_credential(provider_name, &pending.account, creds.to_stored());
            store.switch_account(provider_name, &pending.account);
        })
        .expect("failed to save auth store");

    println!(
        "Authentication successful! Saved provider '{}' account '{}' in {}",
        provider_name,
        pending.account,
        auth_paths.write_path().expect("write path should exist").display(),
    );
}

/// Parse an OAuth callback input in various formats.
///
/// Accepts:
/// - `code#state`
/// - `https://...?code=CODE&state=STATE`
/// - `code state` (space-separated)
fn parse_oauth_callback(input: &str) -> std::result::Result<(String, String), String> {
    let input = input.trim();

    if input.starts_with("http://") || input.starts_with("https://") {
        if let Ok(url) = url::Url::parse(input) {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
                return Ok((code.to_string(), state.to_string()));
            }
        }
        return Err("URL missing 'code' and/or 'state' query parameters.".to_string());
    }

    if let Some((code, state)) = input.split_once('#')
        && !code.is_empty()
        && !state.is_empty()
    {
        return Ok((code.to_string(), state.to_string()));
    }

    if let Some((code, state)) = input.split_once(' ') {
        let code = code.trim();
        let state = state.trim();
        if !code.is_empty() && !state.is_empty() {
            return Ok((code.to_string(), state.to_string()));
        }
    }

    Err(format!(
        "Invalid code format: '{}'. Expected:\n  code#state\n  https://...?code=CODE&state=STATE",
        if input.len() > 40 { &input[..40] } else { input }
    ))
}

// ── Resolve ─────────────────────────────────────────────────────────────

async fn run_resolve(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths, name: &str) {
    let router = build_router(cli, auth_store, auth_paths).await;

    // Try alias first
    if let Some(resolved_id) = ModelAliases::resolve(name) {
        println!("Alias:    {} → {}", name, resolved_id);
    }

    if let Some(model) = router.resolve_model(name) {
        println!("Model:    {}", model.id);
        println!("Provider: {}", model.provider);
        println!("Context:  {}K input, {}K output", model.max_input_tokens / 1000, model.max_output_tokens / 1000);
        println!("Thinking: {}", if model.supports_thinking { "yes" } else { "no" });
        println!("Images:   {}", if model.supports_images { "yes" } else { "no" });
        println!("Tools:    {}", if model.supports_tools { "yes" } else { "no" });
        if let Some(cost) = &model.input_cost_per_mtok {
            println!("Cost:     ${}/Mtok in, ${}/Mtok out", cost, model.output_cost_per_mtok.unwrap_or(0.0));
        }
    } else {
        eprintln!("Model not found: {}", name);
        eprintln!("Hint: configure a provider first, or check `clanker-router models`");
        std::process::exit(1);
    }
}

// ── Status ──────────────────────────────────────────────────────────────

async fn run_status(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths) {
    let router = build_router(cli, auth_store, auth_paths).await;

    println!("clanker-router v{}", env!("CARGO_PKG_VERSION"));
    println!();

    let providers = router.provider_names();
    if providers.is_empty() {
        println!("Providers: none configured");
        println!("\nSet up a provider:");
        println!("  clanker-router auth set-key openai sk-...");
        println!("  export OPENAI_API_KEY=sk-...");
        return;
    }

    println!("Providers:");
    for name in &providers {
        let model_count = router.registry().list_for_provider(name).len();
        println!("  ✓ {} ({} models)", name, model_count);
    }

    println!("\nDefault model: {}", router.default_model());
    println!("Total models:  {}", router.list_models().len());
}

// ── Usage ───────────────────────────────────────────────────────────────

fn run_usage(days: usize, total_only: bool, json: bool) {
    let db_path = default_db_path();
    let db = match clanker_router::RouterDb::open(&db_path) {
        Ok(db) => db,
        Err(e) => {
            eprintln!("Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    if total_only {
        let total = db.usage().total().unwrap();
        if json {
            println!("{}", serde_json::to_string_pretty(&total).unwrap());
        } else {
            print_usage_summary("All-Time", &total);
        }
        return;
    }

    let recent = db.usage().recent_days(days).unwrap();
    if recent.is_empty() {
        println!("No usage data recorded yet.");
        println!("Usage is tracked automatically when requests flow through the router.");
        return;
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&recent).unwrap());
        return;
    }

    // Header
    println!("{:<12} {:>8} {:>10} {:>10} {:>10}", "DATE", "REQUESTS", "INPUT", "OUTPUT", "COST");
    println!("{}", "─".repeat(54));

    let mut total_requests: u32 = 0;
    let mut total_input: u64 = 0;
    let mut total_output: u64 = 0;
    let mut total_cost: f64 = 0.0;

    for day in &recent {
        total_requests += day.requests;
        total_input += day.input_tokens;
        total_output += day.output_tokens;
        total_cost += day.estimated_cost_usd;

        println!(
            "{:<12} {:>8} {:>9}K {:>9}K ${:>8.4}",
            day.date,
            day.requests,
            day.input_tokens / 1000,
            day.output_tokens / 1000,
            day.estimated_cost_usd,
        );
    }

    println!("{}", "─".repeat(54));
    println!(
        "{:<12} {:>8} {:>9}K {:>9}K ${:>8.4}",
        "TOTAL",
        total_requests,
        total_input / 1000,
        total_output / 1000,
        total_cost,
    );

    // Per-provider breakdown for most recent day
    if let Some(today) = recent.first() {
        if !today.by_provider.is_empty() {
            println!("\nToday by provider:");
            for (name, prov) in &today.by_provider {
                println!(
                    "  {:<16} {:>5} reqs  {:>7}K in  {:>7}K out  ${:.4}",
                    name,
                    prov.requests,
                    prov.input_tokens / 1000,
                    prov.output_tokens / 1000,
                    prov.estimated_cost_usd,
                );
                for (model, mu) in &prov.by_model {
                    println!(
                        "    {:<14} {:>5} reqs  {:>7}K in  {:>7}K out  ${:.4}",
                        model,
                        mu.requests,
                        mu.input_tokens / 1000,
                        mu.output_tokens / 1000,
                        mu.estimated_cost_usd,
                    );
                }
            }
        }
    }
}

fn print_usage_summary(label: &str, usage: &clanker_router::db::usage::DailyUsage) {
    println!("{} Usage:", label);
    println!("  Requests:      {}", usage.requests);
    println!("  Input tokens:  {}K", usage.input_tokens / 1000);
    println!("  Output tokens: {}K", usage.output_tokens / 1000);
    println!("  Cache created: {}K", usage.cache_creation_tokens / 1000);
    println!("  Cache reads:   {}K", usage.cache_read_tokens / 1000);
    println!("  Est. cost:     ${:.4}", usage.estimated_cost_usd);

    if !usage.by_provider.is_empty() {
        println!("\n  By provider:");
        for (name, prov) in &usage.by_provider {
            println!(
                "    {} — {} reqs, {}K tokens, ${:.4}",
                name,
                prov.requests,
                (prov.input_tokens + prov.output_tokens) / 1000,
                prov.estimated_cost_usd,
            );
        }
    }
}

// ── Serve (RPC daemon) ──────────────────────────────────────────────────

async fn run_serve(
    cli: &Cli,
    auth_store: &AuthStore,
    auth_paths: &AuthStorePaths,
    daemon: bool,
    proxy_addr: &str,
    proxy_keys: &[String],
) {
    use clanker_router::proxy::ProxyConfig;
    use clanker_router::proxy::iroh_tunnel::IrohTunnel;
    use clanker_router::proxy::iroh_tunnel::{self};
    use clanker_router::proxy::{self};
    use clanker_router::rpc::daemon::DaemonInfo;

    // --daemon: re-exec ourselves as a detached background process,
    // then exit the parent immediately so the shell gets its prompt back.
    if daemon {
        let info_path = clanker_router::rpc::daemon::daemon_info_path();

        // Bail if a daemon is already running
        if let Some(info) = DaemonInfo::load(&info_path) {
            if info.is_alive() {
                eprintln!("Router daemon already running (pid {})", info.pid);
                return;
            }
            DaemonInfo::remove(&info_path);
        }

        let exe = std::env::current_exe().expect("failed to get current executable");
        // Re-exec without --daemon; the child runs in foreground mode
        // but with stdio pointed at /dev/null.
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        args.retain(|a| a != "--daemon");

        let mut cmd = std::process::Command::new(exe);
        cmd.args(&args)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());

        // Create a new session so the child survives terminal close / SIGHUP
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            unsafe {
                cmd.pre_exec(|| {
                    libc::setsid();
                    Ok(())
                });
            }
        }

        match cmd.spawn() {
            Ok(child) => {
                // Wait for daemon.json to appear (up to 5s)
                for _ in 0..50 {
                    std::thread::sleep(std::time::Duration::from_millis(100));
                    if let Some(info) = DaemonInfo::load(&info_path) {
                        if info.is_alive() {
                            eprintln!("Router daemon started (pid {})", info.pid);
                            return;
                        }
                    }
                }
                eprintln!("Router daemon spawned (pid {}) but not ready after 5s", child.id());
            }
            Err(e) => {
                eprintln!("Failed to spawn daemon: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    let router = Arc::new(build_router(cli, auth_store, auth_paths).await);

    if router.provider_names().is_empty() {
        eprintln!("Error: No providers configured.");
        eprintln!("Set an API key first:");
        eprintln!("  clanker-router auth set-key openai sk-...");
        eprintln!("  export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    let provider_summary = router.provider_names().join(", ");

    // Start background cache eviction (if DB + cache are enabled)
    let _eviction_handle = router.start_cache_eviction();

    // OAuth providers refresh on demand during request handling and reload
    // from the managed auth store when credentials change.

    // Parse proxy bind address
    let bind_addr: std::net::SocketAddr = match proxy_addr.parse() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("Invalid proxy address '{}': {}", proxy_addr, e);
            std::process::exit(1);
        }
    };

    // Build the OpenAI-compatible proxy (shares the same router)
    let proxy_config = ProxyConfig {
        bind_addr,
        allowed_keys: proxy_keys.to_vec(),
    };

    // Build the iroh endpoint with both:
    //   1. The existing RPC protocol (for client ↔ router communication)
    //   2. The HTTP tunnel protocol (for remote OpenAI-compatible access)
    let secret_key = iroh::SecretKey::generate(&mut rand::rng());
    let iroh_rpc_alpn = clanker_router::rpc::ALPN.to_vec();
    let iroh_tunnel_alpn = iroh_tunnel::ALPN.to_vec();

    let mdns = iroh::address_lookup::MdnsAddressLookup::builder().service_name(clanker_router::rpc::MDNS_SERVICE);

    let endpoint = match iroh::Endpoint::builder()
        .secret_key(secret_key.clone())
        .alpns(vec![iroh_rpc_alpn.clone(), iroh_tunnel_alpn.clone()])
        .address_lookup(mdns)
        .bind()
        .await
    {
        Ok(ep) => ep,
        Err(e) => {
            tracing::warn!("mDNS unavailable ({}), binding without discovery", e);
            match iroh::Endpoint::builder()
                .secret_key(secret_key)
                .alpns(vec![iroh_rpc_alpn.clone(), iroh_tunnel_alpn.clone()])
                .bind()
                .await
            {
                Ok(ep) => ep,
                Err(e) => {
                    eprintln!("Failed to bind iroh endpoint: {}", e);
                    std::process::exit(1);
                }
            }
        }
    };

    // Create the iroh tunnel handler (forwards to local axum server)
    let tunnel = IrohTunnel::new(bind_addr);

    // Create the RPC handler (shares the same Arc<Router> with the proxy)
    let rpc_handler = clanker_router::rpc::server::RpcHandler::new(Arc::clone(&router));

    // Build the iroh protocol router with both handlers
    let iroh_router = iroh::protocol::Router::builder(endpoint.clone())
        .accept(iroh_rpc_alpn, rpc_handler)
        .accept(iroh_tunnel_alpn, tunnel)
        .spawn();

    let node_id = endpoint.id().to_string();
    let info_path = clanker_router::rpc::daemon::daemon_info_path();

    // Collect bound addresses
    let addrs: Vec<String> = endpoint
        .bound_sockets()
        .into_iter()
        .map(|mut a| {
            if a.ip().is_unspecified() {
                a.set_ip(if a.is_ipv4() {
                    std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
                } else {
                    std::net::IpAddr::V6(std::net::Ipv6Addr::LOCALHOST)
                });
            }
            a.to_string()
        })
        .collect();

    // Write daemon.json
    let info = DaemonInfo {
        node_id: node_id.clone(),
        pid: std::process::id(),
        addrs,
    };
    if let Err(e) = info.save(&info_path) {
        eprintln!("Warning: failed to write daemon info: {}", e);
    }

    eprintln!("Router daemon running");
    eprintln!("  Providers: {}", provider_summary);
    eprintln!("  OpenAI proxy: http://{}", bind_addr);
    eprintln!("    OPENAI_BASE_URL=http://{}/v1", bind_addr);
    if proxy_keys.is_empty() {
        eprintln!("    Auth: disabled (no --proxy-key set)");
    } else {
        eprintln!("    Auth: {} key(s) configured", proxy_keys.len());
    }
    eprintln!("  iroh node: {}", &node_id[..16.min(node_id.len())]);
    eprintln!("    Remote peers can tunnel HTTP over iroh QUIC");
    eprintln!("  Daemon info: {}", info_path.display());
    eprintln!("  Press Ctrl+C to stop.");

    // Clean up daemon.json on shutdown
    let info_path_clone = info_path.clone();
    let _cleanup = scopeguard::ScopeGuard::new((), |_| {
        DaemonInfo::remove(&info_path_clone);
    });

    // Run HTTP proxy + iroh router concurrently (both share the same Arc<Router>)
    tokio::select! {
        result = proxy::serve(Arc::clone(&router), proxy_config) => {
            if let Err(e) = result {
                eprintln!("HTTP proxy error: {}", e);
            }
        }
        _ = tokio::signal::ctrl_c() => {
            eprintln!("\nShutting down...");
        }
    }

    // Graceful shutdown
    iroh_router.shutdown().await.ok();
    DaemonInfo::remove(&info_path);
}

/// Simple RAII cleanup guard.
mod scopeguard {
    pub struct ScopeGuard<T, F: FnOnce(T)> {
        value: Option<T>,
        f: Option<F>,
    }

    impl<T, F: FnOnce(T)> ScopeGuard<T, F> {
        pub fn new(value: T, f: F) -> Self {
            Self {
                value: Some(value),
                f: Some(f),
            }
        }
    }

    impl<T, F: FnOnce(T)> Drop for ScopeGuard<T, F> {
        fn drop(&mut self) {
            if let (Some(v), Some(f)) = (self.value.take(), self.f.take()) {
                f(v);
            }
        }
    }
}

// ── HuggingFace Hub ─────────────────────────────────────────────────────

fn resolve_hf_token(cli: &Cli, auth_store: &AuthStore) -> Option<String> {
    // CLI override
    if let Some(ref key) = cli.api_key {
        return Some(key.clone());
    }
    // Auth store or env (HF_TOKEN)
    resolve_credential("huggingface", None, auth_store, None).map(|c| c.token().to_string())
}

async fn run_hf(cli: &Cli, auth_store: &AuthStore, _auth_paths: &AuthStorePaths, action: &HfAction) {
    let token = resolve_hf_token(cli, auth_store);
    let hub = HubClient::new(token);

    match action {
        HfAction::Search { query, limit, json } => match hub.search(query, Some(*limit)).await {
            Ok(models) => {
                if models.is_empty() {
                    eprintln!("No models found for '{}'", query);
                    return;
                }
                if *json {
                    println!("{}", serde_json::to_string_pretty(&models).unwrap());
                } else {
                    println!("{:<50} {:>10} {:>6} {:<20} {}", "MODEL", "DOWNLOADS", "LIKES", "PIPELINE", "GATED");
                    println!("{}", "─".repeat(100));
                    for m in &models {
                        println!(
                            "{:<50} {:>10} {:>6} {:<20} {}",
                            truncate_str(&m.model_id, 50),
                            m.downloads_display(),
                            m.likes,
                            m.pipeline_tag.as_deref().unwrap_or("—"),
                            if m.is_gated() { "🔒" } else { "" },
                        );
                    }
                    println!("\n{} model(s) found", models.len());
                }
            }
            Err(e) => {
                eprintln!("Search failed: {}", e);
                std::process::exit(1);
            }
        },
        HfAction::Info { model } => match hub.model_info(model).await {
            Ok(info) => {
                println!("Model:     {}", info.model_id);
                if let Some(ref author) = info.author {
                    println!("Author:    {}", author);
                }
                println!("Downloads: {}", info.downloads);
                println!("Likes:     {}", info.likes);
                if let Some(ref pipeline) = info.pipeline_tag {
                    println!("Pipeline:  {}", pipeline);
                }
                if let Some(ref lib) = info.library_name {
                    println!("Library:   {}", lib);
                }
                if !info.tags.is_empty() {
                    println!("Tags:      {}", info.tags.join(", "));
                }
                let gguf_count = info.siblings.iter().filter(|s| s.filename.ends_with(".gguf")).count();
                if gguf_count > 0 {
                    println!("GGUF files: {} (run `clanker-router hf files {}` to list)", gguf_count, model);
                }
            }
            Err(e) => {
                eprintln!("Failed to get model info: {}", e);
                std::process::exit(1);
            }
        },
        HfAction::Files { model } => match hub.list_gguf_files(model).await {
            Ok(files) => {
                if files.is_empty() {
                    eprintln!("No GGUF files found in {}", model);
                    eprintln!(
                        "Hint: Try a GGUF-specific repo (e.g., bartowski/{}-GGUF)",
                        model.split('/').last().unwrap_or(model)
                    );
                    return;
                }
                println!("{:<50} {:>10} {:<10}", "FILENAME", "SIZE", "QUANT");
                println!("{}", "─".repeat(74));
                for f in &files {
                    println!(
                        "{:<50} {:>10} {:<10}",
                        truncate_str(&f.filename, 50),
                        format_hf_bytes(f.size_bytes),
                        f.quantization.as_deref().unwrap_or("—"),
                    );
                }
                println!("\n{} GGUF file(s)", files.len());
                println!("\nPull with: clanker-router hf pull {} --quant Q4_K_M", model);
            }
            Err(e) => {
                eprintln!("Failed to list files: {}", e);
                std::process::exit(1);
            }
        },
        HfAction::Pull {
            model,
            quant,
            ollama,
            ollama_name,
        } => {
            let do_ollama = *ollama || ollama_name.is_some();
            eprintln!("Pulling {} ...", model);

            // Progress callback
            let progress = Box::new(|downloaded: u64, total: u64| {
                if total > 0 {
                    let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
                    eprint!("\r  {} / {} ({}%)", format_hf_bytes(downloaded), format_hf_bytes(total), pct,);
                }
            });

            match hub.pull(model, quant.as_deref(), Some(progress)).await {
                Ok(pulled) => {
                    eprintln!();
                    println!("Downloaded: {}", pulled.local_path.display());
                    println!("Size:       {}", format_hf_bytes(pulled.size_bytes));
                    if let Some(ref q) = pulled.quantization {
                        println!("Quant:      {}", q);
                    }

                    if do_ollama {
                        match hub.register_with_ollama(&pulled, ollama_name.as_deref()).await {
                            Ok(name) => {
                                println!("\nRegistered with Ollama as: {}", name);
                                println!("Run with:  ollama run {}", name);
                                println!("Or route:  clanker-router --provider local --model {} ask \"hello\"", name);
                            }
                            Err(e) => {
                                eprintln!("\nFailed to register with Ollama: {}", e);
                                eprintln!("You can register manually later:");
                                eprintln!("  clanker-router hf ollama {}", model);
                            }
                        }
                    } else {
                        println!("\nTo serve with Ollama:");
                        println!("  clanker-router hf pull {} --ollama", model);
                        println!("Or register an existing pull:");
                        println!("  clanker-router hf ollama {}", model);
                    }
                }
                Err(e) => {
                    eprintln!("\nPull failed: {}", e);
                    std::process::exit(1);
                }
            }
        }
        HfAction::List => {
            let cached = hub.list_cached();
            if cached.is_empty() {
                println!("No cached models.");
                println!("Pull a model with: clanker-router hf pull <model-id>");
                return;
            }
            println!("{:<45} {:<30} {:>10} {:<10}", "MODEL", "FILE", "SIZE", "QUANT");
            println!("{}", "─".repeat(99));
            for m in &cached {
                println!(
                    "{:<45} {:<30} {:>10} {:<10}",
                    truncate_str(&m.model_id, 45),
                    truncate_str(&m.filename, 30),
                    format_hf_bytes(m.size_bytes),
                    m.quantization.as_deref().unwrap_or("—"),
                );
            }
            println!("\nCache dir: {}", hub.cache_dir().display());
        }
        HfAction::Remove { model } => match hub.remove_cached(model) {
            Ok(removed) => {
                if removed.is_empty() {
                    eprintln!("No cached files found for {}", model);
                } else {
                    for f in &removed {
                        println!("Removed: {}", f.display());
                    }
                    println!("\nRemoved {} file(s)", removed.len());
                }
            }
            Err(e) => {
                eprintln!("Remove failed: {}", e);
                std::process::exit(1);
            }
        },
        HfAction::Ollama { model, name } => {
            let cached = hub.list_cached();
            let pulled = cached.iter().find(|m| m.model_id == *model);
            match pulled {
                Some(pulled) => match hub.register_with_ollama(pulled, name.as_deref()).await {
                    Ok(ollama_name) => {
                        println!("Registered with Ollama as: {}", ollama_name);
                        println!("Run with:  ollama run {}", ollama_name);
                    }
                    Err(e) => {
                        eprintln!("Failed to register with Ollama: {}", e);
                        std::process::exit(1);
                    }
                },
                None => {
                    eprintln!("Model '{}' not found in cache.", model);
                    eprintln!("Pull it first: clanker-router hf pull {}", model);
                    std::process::exit(1);
                }
            }
        }
    }
}

fn truncate_str(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

fn format_hf_bytes(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.1} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{} B", bytes)
    }
}

// ── TUI ─────────────────────────────────────────────────────────────────

async fn run_tui(cli: &Cli, auth_store: &AuthStore, auth_paths: &AuthStorePaths, system: Option<String>) {
    let router = build_router(cli, auth_store, auth_paths).await;

    if router.provider_names().is_empty() {
        exit_no_providers(cli);
    }

    let model_names: Vec<String> = router.list_models().iter().map(|m| m.id.clone()).collect();

    if let Err(e) = tui::run(router, cli.model.clone(), model_names, system).await {
        eprintln!("TUI error: {}", e);
        std::process::exit(1);
    }
}
