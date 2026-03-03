//! clankers-router CLI + TUI
//!
//! Standalone binary for managing LLM provider routing, credentials,
//! model discovery, and interactive chat.

mod tui;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use clankers_router::Router;
use clankers_router::auth::AuthStore;
use clankers_router::auth::StoredCredential;
use clankers_router::auth::env_var_for_provider;
use clankers_router::auth::resolve_credential;
use clankers_router::backends::huggingface::HubClient;
use clankers_router::backends::openai_compat::OpenAICompatConfig;
use clankers_router::backends::openai_compat::OpenAICompatProvider;
use clankers_router::credential_pool::CredentialPool;
use clankers_router::credential_pool::SelectionStrategy;
use clankers_router::model::ModelAliases;
use clankers_router::oauth;
use clankers_router::provider::CompletionRequest;
use clankers_router::provider::Provider;
use clankers_router::streaming::ContentDelta;
use clankers_router::streaming::StreamEvent;
use tokio::sync::mpsc;

// ── CLI definition ──────────────────────────────────────────────────────

#[derive(Parser)]
#[command(
    name = "clankers-router",
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

    /// Auth store path
    #[arg(long)]
    auth_file: Option<PathBuf>,

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
    /// Show credential status for all providers
    Status,
    /// Log in to Anthropic via OAuth (Claude Max)
    Login {
        /// Account name
        #[arg(long, default_value = "default")]
        account: String,
        /// Complete login with code from browser (code#state or callback URL)
        #[arg(long)]
        code: Option<String>,
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
    dirs::config_dir().unwrap_or_else(|| PathBuf::from(".")).join("clankers-router").join("auth.json")
}

fn resolve_auth_path(cli: &Cli) -> PathBuf {
    cli.auth_file.clone().unwrap_or_else(default_auth_path)
}

// ── Provider construction ───────────────────────────────────────────────

fn build_providers(cli: &Cli, auth_store: &AuthStore) -> Vec<Arc<dyn Provider>> {
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

        // Add env var as an extra source (if not already present)
        if let Some(env_var) = env_var_for_provider(name)
            && let Ok(key) = std::env::var(env_var)
            && !key.is_empty()
        {
            let env_label = format!("env:{}", env_var);
            let already_has = creds.iter().any(|(_, c)| c.token() == key);
            if !already_has {
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
            use clankers_router::backends::anthropic::AnthropicProvider;
            use clankers_router::backends::anthropic::Credential;
            let base_url = if cli.provider.as_deref() == Some("anthropic") {
                cli.api_base.clone()
            } else {
                None
            };

            if creds.len() > 1 {
                // Multi-account: build a credential pool
                let pool = CredentialPool::new(creds, SelectionStrategy::Failover);
                tracing::info!("anthropic: {} account(s) in pool", pool.len());
                providers.push(AnthropicProvider::with_pool(pool, base_url));
            } else {
                // Single account: legacy path
                let cred = &creds[0].1;
                let anthropic_cred = if cred.is_oauth() {
                    Credential::OAuth(cred.token().to_string())
                } else {
                    Credential::ApiKey(cred.token().to_string())
                };
                providers.push(AnthropicProvider::new(anthropic_cred, base_url));
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
        let models = vec![clankers_router::Model {
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

    providers
}

fn default_db_path() -> PathBuf {
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("clankers-router").join("router.db")
}

fn build_router(cli: &Cli, auth_store: &AuthStore) -> Router {
    let db_path = default_db_path();
    let mut router = match clankers_router::RouterDb::open(&db_path) {
        Ok(db) => {
            tracing::debug!("Opened router database at {}", db_path.display());
            Router::with_db(&cli.model, db)
        }
        Err(e) => {
            tracing::warn!("Failed to open router database ({}), running without persistence", e);
            Router::new(&cli.model)
        }
    };
    for provider in build_providers(cli, auth_store) {
        router.register_provider(provider);
    }

    // Enable default fallback chains so rate-limited providers
    // automatically fall over to alternatives.
    router.set_fallbacks(clankers_router::FallbackConfig::with_defaults());

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
            .with_env_filter(format!("clankers_router=debug{iroh_filters}"))
            .with_writer(std::io::stderr)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(format!("clankers_router=warn{iroh_filters}"))
            .with_writer(std::io::stderr)
            .init();
    }

    let auth_path = resolve_auth_path(&cli);
    let auth_store = AuthStore::load(&auth_path);

    match &cli.command {
        None | Some(Commands::Chat { .. }) => {
            let system = match &cli.command {
                Some(Commands::Chat { system }) => system.clone(),
                _ => None,
            };
            run_tui(&cli, &auth_store, system).await;
        }
        Some(Commands::Ask {
            prompt,
            system,
            max_tokens,
            temperature,
            format,
        }) => {
            run_ask(&cli, &auth_store, prompt, system.clone(), *max_tokens, *temperature, format).await;
        }
        Some(Commands::Models { provider, json }) => {
            run_models(&cli, &auth_store, provider.as_deref(), *json);
        }
        Some(Commands::Auth { action }) => {
            run_auth(&auth_path, action).await;
        }
        Some(Commands::Resolve { name }) => {
            run_resolve(&cli, &auth_store, name);
        }
        Some(Commands::Status) => {
            run_status(&cli, &auth_store);
        }
        Some(Commands::Usage { days, total, json }) => {
            run_usage(*days, *total, *json);
        }
        Some(Commands::Hf { action }) => {
            run_hf(&cli, &auth_store, action).await;
        }
        Some(Commands::Serve {
            daemon,
            proxy_addr,
            proxy_key,
        }) => {
            run_serve(&cli, &auth_store, *daemon, proxy_addr, proxy_key).await;
        }
    }
}

// ── Ask (non-interactive) ───────────────────────────────────────────────

async fn run_ask(
    cli: &Cli,
    auth_store: &AuthStore,
    prompt: &str,
    system: Option<String>,
    max_tokens: Option<usize>,
    temperature: Option<f64>,
    format: &OutputFormat,
) {
    let router = build_router(cli, auth_store);

    if router.provider_names().is_empty() {
        eprintln!("Error: No providers configured. Set an API key:");
        eprintln!("  clankers-router auth set-key openai sk-...");
        eprintln!("  export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    let request = CompletionRequest {
        model: cli.model.clone(),
        messages: vec![serde_json::json!({"role": "user", "content": prompt})],
        system_prompt: system,
        max_tokens,
        temperature,
        tools: vec![],
        thinking: None,
            extra_params: Default::default(),
    };

    let (tx, mut rx) = mpsc::channel(64);

    let complete_handle = tokio::spawn(async move { router.complete(request, tx).await });

    let mut full_text = String::new();
    let mut usage = clankers_router::provider::Usage::default();

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

fn run_models(cli: &Cli, auth_store: &AuthStore, provider_filter: Option<&str>, json: bool) {
    let router = build_router(cli, auth_store);
    let models = if let Some(prov) = provider_filter {
        router.registry().list_for_provider(prov).into_iter().cloned().collect::<Vec<_>>()
    } else {
        router.list_models().into_iter().cloned().collect::<Vec<_>>()
    };

    if models.is_empty() {
        eprintln!("No models available. Configure a provider first:");
        eprintln!("  clankers-router auth set-key openai sk-...");
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

async fn run_auth(auth_path: &PathBuf, action: &AuthAction) {
    match action {
        AuthAction::Login { account, code } => {
            run_auth_login(auth_path, account, code.as_deref()).await;
        }
        AuthAction::Status => {
            let store = AuthStore::load(auth_path);
            let summary = store.summary();
            if summary.contains("No credentials") {
                println!("{}", summary);
                println!("\nEnvironment variables:");
                let providers = [
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
                ];
                for p in providers {
                    if let Some(var) = env_var_for_provider(p) {
                        let status = if std::env::var(var).is_ok() {
                            "✓ set"
                        } else {
                            "· not set"
                        };
                        println!("  {} {} ({})", status, p, var);
                    }
                }
            } else {
                print!("{}", summary);
            }
            println!("\nAuth file: {}", auth_path.display());
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

            let mut store = AuthStore::load(auth_path);
            store.set_credential(provider, account, StoredCredential::ApiKey {
                api_key,
                label: label.clone(),
            });
            store.save(auth_path).expect("failed to save auth store");
            println!("Saved {} key for account '{}' in {}", provider, account, auth_path.display());
        }
        AuthAction::Remove { provider, account } => {
            let mut store = AuthStore::load(auth_path);
            if store.remove_account(&provider, &account) {
                store.save(auth_path).expect("failed to save");
                println!("Removed {}/{}", provider, account);
            } else {
                eprintln!("Account not found: {}/{}", provider, account);
                std::process::exit(1);
            }
        }
        AuthAction::Switch { provider, account } => {
            let mut store = AuthStore::load(auth_path);
            if store.switch_account(&provider, &account) {
                store.save(auth_path).expect("failed to save");
                println!("Switched {} to account '{}'", provider, account);
            } else {
                eprintln!("Account not found: {}/{}", provider, account);
                std::process::exit(1);
            }
        }
        AuthAction::Accounts { provider } => {
            let store = AuthStore::load(auth_path);
            let providers: Vec<&str> = if let Some(p) = provider {
                vec![p.as_str()]
            } else {
                store.configured_providers()
            };

            if providers.is_empty() {
                println!("No accounts configured.");
                return;
            }

            for p in providers {
                println!("{}:", p);
                for info in store.list_accounts(p) {
                    let marker = if info.is_active { "▸" } else { " " };
                    let kind = if info.is_oauth { "oauth" } else { "api-key" };
                    let label = info.label.as_ref().map(|l| format!(" — {}", l)).unwrap_or_default();
                    let expired = if info.is_expired { " (expired)" } else { "" };
                    println!("  {} {} [{}]{}{}", marker, info.name, kind, label, expired);
                }
            }
        }
    }
}

// ── OAuth login ─────────────────────────────────────────────────────────

async fn run_auth_login(auth_path: &PathBuf, account: &str, code_input: Option<&str>) {
    let verifier_path = auth_path.parent().unwrap_or(std::path::Path::new(".")).join(".login_verifier");

    let input = if let Some(input) = code_input {
        // --code was passed directly, recover verifier from disk
        input.to_string()
    } else {
        // Step 1: generate auth URL and prompt user
        let (url, verifier) = oauth::build_auth_url();

        println!("Logging in as account: {}", account);

        // Try to open browser
        if open::that_detached(&url).is_ok() {
            println!("Opening browser automatically...\n");
        } else {
            println!("Could not open browser automatically.\n");
        }

        // Print clickable hyperlink (OSC 8)
        println!("Ctrl+Click or open this URL in your browser:\n\n  \x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\n", url, url);
        println!(
            "After authorizing, paste the code or callback URL.\n\
             Accepted formats:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE\n"
        );

        // Persist verifier so --code can be used from another invocation
        if let Some(parent) = verifier_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        std::fs::write(&verifier_path, &verifier).ok();

        // Read input from stdin
        let mut buf = String::new();
        std::io::stdin().read_line(&mut buf).expect("failed to read input");
        buf.trim().to_string()
    };

    // Parse code + state
    let (code, state) = match parse_oauth_callback(&input) {
        Ok(pair) => pair,
        Err(msg) => {
            eprintln!("Error: {}", msg);
            std::process::exit(1);
        }
    };

    // Load verifier
    let verifier = match std::fs::read_to_string(&verifier_path) {
        Ok(v) => v,
        Err(_) => {
            eprintln!("Error: No login in progress. Run `clankers-router auth login` first.");
            std::process::exit(1);
        }
    };

    // Exchange code for credentials
    let creds = match oauth::exchange_code(&code, &state, &verifier).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Login failed: {}", e);
            std::process::exit(1);
        }
    };

    // Clean up verifier
    std::fs::remove_file(&verifier_path).ok();

    // Save to auth store
    let mut store = AuthStore::load(auth_path);
    store.set_credential("anthropic", account, creds.to_stored());
    if store.providers.get("anthropic").and_then(|p| p.active_account.as_deref()).is_none() {
        store.switch_account("anthropic", account);
    }
    store.save(auth_path).expect("failed to save auth store");

    println!("Authentication successful! Saved as account '{}' in {}", account, auth_path.display());
}

/// Parse an OAuth callback input in various formats.
///
/// Accepts:
/// - `code#state`
/// - `https://...?code=CODE&state=STATE`
/// - `code state` (space-separated)
fn parse_oauth_callback(input: &str) -> std::result::Result<(String, String), String> {
    let input = input.trim();

    // URL format
    if input.starts_with("http://") || input.starts_with("https://") {
        if let Ok(url) = url::Url::parse(input) {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
                return Ok((code.to_string(), state.to_string()));
            }
        }
        return Err("URL missing 'code' and/or 'state' query parameters.".to_string());
    }

    // code#state format
    if let Some((code, state)) = input.split_once('#') {
        if !code.is_empty() && !state.is_empty() {
            return Ok((code.to_string(), state.to_string()));
        }
    }

    // space-separated
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

fn run_resolve(cli: &Cli, auth_store: &AuthStore, name: &str) {
    let router = build_router(cli, auth_store);

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
        eprintln!("Hint: configure a provider first, or check `clankers-router models`");
        std::process::exit(1);
    }
}

// ── Status ──────────────────────────────────────────────────────────────

fn run_status(cli: &Cli, auth_store: &AuthStore) {
    let router = build_router(cli, auth_store);

    println!("clankers-router v{}", env!("CARGO_PKG_VERSION"));
    println!();

    let providers = router.provider_names();
    if providers.is_empty() {
        println!("Providers: none configured");
        println!("\nSet up a provider:");
        println!("  clankers-router auth set-key openai sk-...");
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
    let db = match clankers_router::RouterDb::open(&db_path) {
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

fn print_usage_summary(label: &str, usage: &clankers_router::db::usage::DailyUsage) {
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

async fn run_serve(cli: &Cli, auth_store: &AuthStore, daemon: bool, proxy_addr: &str, proxy_keys: &[String]) {
    use clankers_router::proxy::ProxyConfig;
    use clankers_router::proxy::iroh_tunnel::IrohTunnel;
    use clankers_router::proxy::iroh_tunnel::{self};
    use clankers_router::proxy::{self};
    use clankers_router::rpc::daemon::DaemonInfo;

    // --daemon: re-exec ourselves as a detached background process,
    // then exit the parent immediately so the shell gets its prompt back.
    if daemon {
        let info_path = clankers_router::rpc::daemon::daemon_info_path();

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

    let router = Arc::new(build_router(cli, auth_store));

    if router.provider_names().is_empty() {
        eprintln!("Error: No providers configured.");
        eprintln!("Set an API key first:");
        eprintln!("  clankers-router auth set-key openai sk-...");
        eprintln!("  export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    let provider_summary = router.provider_names().join(", ");

    // Start background cache eviction (if DB + cache are enabled)
    let _eviction_handle = router.start_cache_eviction();

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
    //   1. The existing RPC protocol (for clankers ↔ router communication)
    //   2. The HTTP tunnel protocol (for remote OpenAI-compatible access)
    let secret_key = iroh::SecretKey::generate(&mut rand::rng());
    let iroh_rpc_alpn = clankers_router::rpc::ALPN.to_vec();
    let iroh_tunnel_alpn = iroh_tunnel::ALPN.to_vec();

    let mdns = iroh::address_lookup::MdnsAddressLookup::builder().service_name(clankers_router::rpc::MDNS_SERVICE);

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
    let rpc_handler = clankers_router::rpc::server::RpcHandler::new(Arc::clone(&router));

    // Build the iroh protocol router with both handlers
    let iroh_router = iroh::protocol::Router::builder(endpoint.clone())
        .accept(iroh_rpc_alpn, rpc_handler)
        .accept(iroh_tunnel_alpn, tunnel)
        .spawn();

    let node_id = endpoint.id().to_string();
    let info_path = clankers_router::rpc::daemon::daemon_info_path();

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

    // Graceful shutdown of iroh router
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

async fn run_hf(cli: &Cli, auth_store: &AuthStore, action: &HfAction) {
    let token = resolve_hf_token(cli, auth_store);
    let hub = HubClient::new(token);

    match action {
        HfAction::Search { query, limit, json } => {
            match hub.search(query, Some(*limit)).await {
                Ok(models) => {
                    if models.is_empty() {
                        eprintln!("No models found for '{}'", query);
                        return;
                    }
                    if *json {
                        println!("{}", serde_json::to_string_pretty(&models).unwrap());
                    } else {
                        println!(
                            "{:<50} {:>10} {:>6} {:<20} {}",
                            "MODEL", "DOWNLOADS", "LIKES", "PIPELINE", "GATED"
                        );
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
            }
        }
        HfAction::Info { model } => {
            match hub.model_info(model).await {
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
                        println!("GGUF files: {} (run `clankers-router hf files {}` to list)", gguf_count, model);
                    }
                }
                Err(e) => {
                    eprintln!("Failed to get model info: {}", e);
                    std::process::exit(1);
                }
            }
        }
        HfAction::Files { model } => {
            match hub.list_gguf_files(model).await {
                Ok(files) => {
                    if files.is_empty() {
                        eprintln!("No GGUF files found in {}", model);
                        eprintln!("Hint: Try a GGUF-specific repo (e.g., bartowski/{}-GGUF)", model.split('/').last().unwrap_or(model));
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
                    println!("\nPull with: clankers-router hf pull {} --quant Q4_K_M", model);
                }
                Err(e) => {
                    eprintln!("Failed to list files: {}", e);
                    std::process::exit(1);
                }
            }
        }
        HfAction::Pull { model, quant, ollama, ollama_name } => {
            let do_ollama = *ollama || ollama_name.is_some();
            eprintln!("Pulling {} ...", model);

            // Progress callback
            let progress = Box::new(|downloaded: u64, total: u64| {
                if total > 0 {
                    let pct = (downloaded as f64 / total as f64 * 100.0) as u32;
                    eprint!(
                        "\r  {} / {} ({}%)",
                        format_hf_bytes(downloaded),
                        format_hf_bytes(total),
                        pct,
                    );
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
                                println!("Or route:  clankers-router --provider local --model {} ask \"hello\"", name);
                            }
                            Err(e) => {
                                eprintln!("\nFailed to register with Ollama: {}", e);
                                eprintln!("You can register manually later:");
                                eprintln!("  clankers-router hf ollama {}", model);
                            }
                        }
                    } else {
                        println!("\nTo serve with Ollama:");
                        println!("  clankers-router hf pull {} --ollama", model);
                        println!("Or register an existing pull:");
                        println!("  clankers-router hf ollama {}", model);
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
                println!("Pull a model with: clankers-router hf pull <model-id>");
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
        HfAction::Remove { model } => {
            match hub.remove_cached(model) {
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
            }
        }
        HfAction::Ollama { model, name } => {
            let cached = hub.list_cached();
            let pulled = cached.iter().find(|m| m.model_id == *model);
            match pulled {
                Some(pulled) => {
                    match hub.register_with_ollama(pulled, name.as_deref()).await {
                        Ok(ollama_name) => {
                            println!("Registered with Ollama as: {}", ollama_name);
                            println!("Run with:  ollama run {}", ollama_name);
                        }
                        Err(e) => {
                            eprintln!("Failed to register with Ollama: {}", e);
                            std::process::exit(1);
                        }
                    }
                }
                None => {
                    eprintln!("Model '{}' not found in cache.", model);
                    eprintln!("Pull it first: clankers-router hf pull {}", model);
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

async fn run_tui(cli: &Cli, auth_store: &AuthStore, system: Option<String>) {
    let router = build_router(cli, auth_store);

    if router.provider_names().is_empty() {
        eprintln!("Error: No providers configured.");
        eprintln!("Set an API key first:");
        eprintln!("  clankers-router auth set-key openai sk-...");
        eprintln!("  export OPENAI_API_KEY=sk-...");
        std::process::exit(1);
    }

    let model_names: Vec<String> = router.list_models().iter().map(|m| m.id.clone()).collect();

    if let Err(e) = tui::run(router, cli.model.clone(), model_names, system).await {
        eprintln!("TUI error: {}", e);
        std::process::exit(1);
    }
}
