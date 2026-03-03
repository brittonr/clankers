use clankers::error::ConfigSnafu;
use clankers::error::Result;
use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;
use tracing::info;

#[derive(Parser, Debug)]
#[command(
    name = "clankers",
    about = "clankers — a Rust terminal coding agent",
    version,
    long_about = None,
)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    /// Print mode: execute a single prompt and exit
    #[arg(short, long, value_name = "PROMPT")]
    print: Option<String>,

    /// Output mode
    #[arg(long, value_enum, default_value_t = OutputMode::Interactive)]
    mode: OutputMode,

    /// Model to use (overrides settings)
    #[arg(long, value_name = "MODEL")]
    model: Option<String>,

    /// Provider to use (anthropic, openai, etc.)
    #[arg(long, value_name = "PROVIDER")]
    provider: Option<String>,

    /// Maximum output tokens
    #[arg(long, value_name = "TOKENS")]
    max_tokens: Option<usize>,

    /// Temperature (0.0-1.0)
    #[arg(long, value_name = "TEMP")]
    temperature: Option<f32>,

    /// Top-p sampling (0.0-1.0)
    #[arg(long, value_name = "P")]
    top_p: Option<f32>,

    /// Top-k sampling
    #[arg(long, value_name = "K")]
    top_k: Option<u32>,

    /// System prompt (overrides default)
    #[arg(long, value_name = "PROMPT")]
    system_prompt: Option<String>,

    /// Append to system prompt
    #[arg(long, value_name = "TEXT")]
    system_prompt_suffix: Option<String>,

    /// Prepend to system prompt
    #[arg(long, value_name = "TEXT")]
    system_prompt_prefix: Option<String>,

    /// Load system prompt from file
    #[arg(long, value_name = "FILE")]
    system_prompt_file: Option<String>,

    /// Allowed tools (comma-separated, or "all")
    #[arg(long, value_name = "TOOLS")]
    tools: Option<String>,

    /// Attach files for context (can be specified multiple times)
    #[arg(long, value_name = "FILE")]
    attach: Vec<String>,

    /// Working directory
    #[arg(long, value_name = "DIR")]
    cwd: Option<String>,

    /// Resume a previous session by ID
    #[arg(long, value_name = "SESSION_ID")]
    resume: Option<String>,

    /// Continue the most recent session
    #[arg(long, short = 'c')]
    r#continue: bool,

    /// Disable git worktree isolation
    #[arg(long)]
    no_worktree: bool,

    /// (Deprecated) Zellij flags — pane management is now built into the TUI
    #[arg(long, hide = true)]
    zellij: bool,
    #[arg(long, hide = true)]
    swarm: bool,
    #[arg(long, hide = true)]
    no_zellij: bool,

    /// Disable session persistence
    #[arg(long)]
    no_session: bool,

    /// Disable prompt caching
    #[arg(long)]
    no_cache: bool,

    /// Agent definition to use
    #[arg(long, value_name = "AGENT")]
    agent: Option<String>,

    /// Agent scope for discovery
    #[arg(long, value_enum)]
    agent_scope: Option<AgentScopeArg>,

    /// Enable extended thinking
    #[arg(long)]
    thinking: bool,

    /// Thinking budget tokens
    #[arg(long, value_name = "TOKENS")]
    thinking_budget: Option<usize>,

    /// Read prompt from stdin
    #[arg(long)]
    stdin: bool,

    /// Output file for print mode
    #[arg(short = 'o', long, value_name = "FILE")]
    output: Option<String>,

    /// Account name to use (for multi-account setups)
    #[arg(long, value_name = "NAME")]
    account: Option<String>,

    /// API key override (for testing)
    #[arg(long, value_name = "KEY", env = "CLANKERS_API_KEY")]
    api_key: Option<String>,

    /// API base URL override
    #[arg(long, value_name = "URL")]
    api_base: Option<String>,

    /// Request timeout in seconds
    #[arg(long, value_name = "SECONDS")]
    timeout: Option<u64>,

    /// Maximum cost budget in USD
    #[arg(long, value_name = "DOLLARS")]
    max_cost: Option<f64>,

    /// Maximum loop iterations
    #[arg(long, value_name = "N", default_value_t = 25)]
    max_iterations: usize,

    /// Confirm before executing tool calls
    #[arg(long)]
    confirm: bool,

    /// Dry run: show tool calls without executing
    #[arg(long)]
    dry_run: bool,

    /// Enable auto-approval of tool calls
    #[arg(long)]
    auto_approve: bool,

    /// Load skill by name
    #[arg(long, value_name = "SKILL")]
    skill: Vec<String>,

    /// Skill directory path
    #[arg(long, value_name = "DIR")]
    skill_dir: Option<String>,

    /// Environment variables to set (KEY=VALUE)
    #[arg(long, value_name = "KEY=VALUE")]
    env: Vec<String>,

    /// Stream output in real-time
    #[arg(long)]
    stream: bool,

    /// Disable streaming
    #[arg(long)]
    no_stream: bool,

    /// Show token usage stats
    #[arg(long)]
    stats: bool,

    /// Log file path
    #[arg(long, value_name = "FILE")]
    log_file: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, value_name = "LEVEL")]
    log_level: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(ValueEnum, Clone, Debug)]
enum OutputMode {
    /// Interactive TUI mode
    Interactive,
    /// Print mode (single prompt and exit)
    Print,
    /// JSON output
    Json,
    /// Markdown output
    Markdown,
    /// Plain text output
    Plain,
}

#[derive(ValueEnum, Clone, Debug)]
enum AgentScopeArg {
    /// User-level agents only
    User,
    /// Project-level agents only
    Project,
    /// Both user and project agents
    Both,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage sessions
    Session {
        #[command(subcommand)]
        action: SessionAction,
    },
    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Authenticate with a provider
    Auth {
        #[command(subcommand)]
        action: AuthAction,
    },
    /// Manage skills
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Manage agent definitions
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },
    /// Show version and build information
    Version {
        /// Show detailed version info
        #[arg(long)]
        verbose: bool,
    },
    /// Manage plugins
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Run diagnostics and health checks
    Doctor {
        /// Fix common issues automatically
        #[arg(long)]
        fix: bool,
    },
    /// Share the current Zellij session over the network
    Share {
        /// Read-only mode (remote guests cannot type)
        #[arg(long)]
        read_only: bool,
    },
    /// Join a remote shared Zellij session
    Join {
        /// Remote node ID (from `clankers share` output)
        node_id: String,
        /// Pre-shared key (hex, from `clankers share` output)
        psk: String,
    },
    /// Peer-to-peer RPC via iroh
    Rpc {
        /// Path to identity key file (default: ~/.clankers/agent/identity.key)
        #[arg(long, value_name = "FILE")]
        identity: Option<String>,

        #[command(subcommand)]
        action: RpcAction,
    },
    /// Run as a headless daemon — listens on iroh (+ optional Matrix) for incoming messages
    Daemon {
        /// Capability tags to advertise (comma-separated)
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Allow all iroh peers (no allowlist check)
        #[arg(long)]
        allow_all: bool,
        /// Enable Matrix bridge
        #[arg(long)]
        matrix: bool,
        /// Heartbeat interval in seconds (0 = disabled)
        #[arg(long, default_value_t = 60)]
        heartbeat: u64,
        /// Maximum concurrent sessions
        #[arg(long, default_value_t = 32)]
        max_sessions: usize,
    },
    /// Run the merge daemon (watches for completed workers and auto-merges)
    MergeDaemon {
        /// Polling interval in seconds
        #[arg(long, default_value_t = 5)]
        interval: u64,
        /// Run one cycle and exit (instead of continuous loop)
        #[arg(long)]
        once: bool,
    },
}

#[derive(Subcommand, Debug)]
enum RpcAction {
    /// Show this node's identity (public key / EndpointId)
    Id,
    /// Start the RPC server (listens for incoming connections)
    Start {
        /// Also enable agent prompt handling
        #[arg(long)]
        with_agent: bool,
        /// Capability tags to advertise (comma-separated, e.g. "gpu,code-review")
        #[arg(long, value_delimiter = ',')]
        tags: Vec<String>,
        /// Allow all peers (no allowlist check)
        #[arg(long)]
        allow_all: bool,
        /// Enable background heartbeat (probes peers periodically)
        #[arg(long)]
        heartbeat: bool,
        /// Heartbeat interval in seconds (default: 60)
        #[arg(long, default_value_t = 60)]
        heartbeat_interval: u64,
    },
    /// Ping a remote clankers instance
    Ping {
        /// Remote node ID (public key)
        node_id: String,
    },
    /// Get version info from a remote clankers instance
    Version {
        /// Remote node ID (public key)
        node_id: String,
    },
    /// Get status from a remote clankers instance
    Status {
        /// Remote node ID (public key)
        node_id: String,
    },
    /// Send a prompt to a remote clankers instance (streams output in real-time)
    Prompt {
        /// Remote node ID (public key)
        node_id: String,
        /// The prompt text
        text: String,
    },
    /// Manage known peers
    Peers {
        #[command(subcommand)]
        action: PeerAction,
    },
    /// Manage the peer allowlist (who can connect to your server)
    Allow {
        /// Peer node ID to allow
        node_id: String,
    },
    /// Remove a peer from the allowlist
    Deny {
        /// Peer node ID to deny
        node_id: String,
    },
    /// Show the current allowlist
    Allowed,
    /// Discover peers: probe all known peers and scan LAN via mDNS
    Discover {
        /// Also scan local network via mDNS for new peers
        #[arg(long)]
        mdns: bool,
        /// mDNS scan duration in seconds (default: 5)
        #[arg(long, default_value_t = 5)]
        scan_secs: u64,
    },
    /// Send a file to a remote peer
    SendFile {
        /// Remote node ID (public key)
        node_id: String,
        /// Path to the local file to send
        file: String,
    },
    /// Receive (download) a file from a remote peer
    RecvFile {
        /// Remote node ID (public key)
        node_id: String,
        /// Remote file path
        remote_path: String,
        /// Local path to save to (default: current dir + filename)
        #[arg(short, long)]
        output: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum PeerAction {
    /// List known peers
    List,
    /// Add a peer to the registry
    Add {
        /// Peer node ID (public key)
        node_id: String,
        /// Human-readable name for this peer
        name: String,
    },
    /// Remove a peer from the registry
    Remove {
        /// Peer node ID (or name)
        peer: String,
    },
    /// Probe a peer and update its capabilities
    Probe {
        /// Peer node ID (or name). Use "all" to probe all peers.
        peer: String,
    },
}

#[derive(Subcommand, Debug)]
enum SessionAction {
    /// List recent sessions
    List {
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Show all sessions
        #[arg(long)]
        all: bool,
    },
    /// Show session details
    Show {
        session_id: String,
        /// Show full conversation history
        #[arg(long)]
        full: bool,
    },
    /// Delete a session
    Delete {
        session_id: String,
        /// Delete without confirmation
        #[arg(long)]
        force: bool,
    },
    /// Delete all sessions
    DeleteAll {
        /// Delete without confirmation
        #[arg(long)]
        force: bool,
    },
    /// Export session to file
    Export {
        session_id: String,
        /// Output file path
        #[arg(short = 'o', long, value_name = "FILE")]
        output: Option<String>,
        /// Export format
        #[arg(long, value_enum, default_value_t = ExportFormat::Json)]
        format: ExportFormat,
    },
    /// Import session from file
    Import {
        /// Input file path
        file: String,
    },
}

#[derive(ValueEnum, Clone, Debug)]
enum ExportFormat {
    Json,
    Markdown,
    Text,
}

#[derive(Subcommand, Debug)]
enum ConfigAction {
    /// Show current configuration
    Show {
        /// Show as JSON
        #[arg(long)]
        json: bool,
    },
    /// Open settings file in editor
    Edit {
        /// Edit project settings instead of global
        #[arg(long)]
        project: bool,
    },
    /// Show resolved paths
    Paths,
    /// Get a config value
    Get {
        /// Config key (dot notation)
        key: String,
    },
    /// Set a config value
    Set {
        /// Config key (dot notation)
        key: String,
        /// Config value
        value: String,
        /// Set in project config instead of global
        #[arg(long)]
        project: bool,
    },
    /// Unset a config value
    Unset {
        /// Config key (dot notation)
        key: String,
        /// Unset in project config instead of global
        #[arg(long)]
        project: bool,
    },
    /// Initialize default configuration
    Init {
        /// Force overwrite existing config
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum AuthAction {
    /// Authenticate with a provider (OAuth)
    Login {
        /// Provider name (anthropic, openai, etc.)
        provider: Option<String>,
        /// Account name (e.g. "work", "personal"). Defaults to "default".
        #[arg(long, value_name = "NAME")]
        account: Option<String>,
        /// Authorization code#state from the OAuth callback (skips interactive prompt)
        #[arg(long, value_name = "CODE")]
        code: Option<String>,
    },
    /// Show current auth status
    Status {
        /// Show for all providers
        #[arg(long)]
        all: bool,
    },
    /// Remove stored credentials
    Logout {
        /// Provider name (anthropic, openai, etc.)
        provider: Option<String>,
        /// Account name to remove
        #[arg(long, value_name = "NAME")]
        account: Option<String>,
        /// Remove all credentials
        #[arg(long)]
        all: bool,
    },
    /// Switch the active account
    Switch {
        /// Account name to switch to
        account: String,
    },
    /// List all accounts
    Accounts,
    /// Set API key directly
    SetKey {
        /// Provider name
        provider: String,
        /// API key (will prompt if not provided)
        #[arg(long, value_name = "KEY")]
        key: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum SkillAction {
    /// List available skills
    List {
        /// Show detailed information
        #[arg(long)]
        verbose: bool,
    },
    /// Show skill details
    Show {
        /// Skill name
        name: String,
    },
    /// Create a new skill
    New {
        /// Skill name
        name: String,
        /// Create in project skills directory
        #[arg(long)]
        project: bool,
    },
    /// Install a skill from URL or path
    Install {
        /// URL or path to skill
        source: String,
        /// Skill name (defaults to inferred)
        #[arg(long)]
        name: Option<String>,
    },
    /// Uninstall a skill
    Uninstall {
        /// Skill name
        name: String,
    },
    /// Update all skills
    Update {
        /// Update specific skill
        name: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
enum AgentAction {
    /// List available agent definitions
    List {
        /// Show detailed information
        #[arg(long)]
        verbose: bool,
    },
    /// Show agent definition details
    Show {
        /// Agent name
        name: String,
    },
    /// Create a new agent definition
    New {
        /// Agent name
        name: String,
        /// Create in project agents directory
        #[arg(long)]
        project: bool,
    },
    /// Edit an agent definition
    Edit {
        /// Agent name
        name: String,
    },
    /// Delete an agent definition
    Delete {
        /// Agent name
        name: String,
        /// Delete without confirmation
        #[arg(long)]
        force: bool,
    },
}

#[derive(Subcommand, Debug)]
enum PluginAction {
    /// List discovered plugins
    List {
        /// Show detailed information
        #[arg(long)]
        verbose: bool,
    },
    /// Show plugin details
    Show {
        /// Plugin name
        name: String,
    },
    /// Install a plugin from a path
    Install {
        /// Path to plugin directory (must contain plugin.json)
        source: String,
        /// Install to project plugins instead of global
        #[arg(long)]
        project: bool,
    },
    /// Uninstall a plugin
    Uninstall {
        /// Plugin name
        name: String,
        /// Uninstall from project plugins instead of global
        #[arg(long)]
        project: bool,
    },
}

/// Parse OAuth callback input in various formats:
/// - `code#state` (the compact format)
/// - `https://...?code=CODE&state=STATE` (callback URL pasted from browser)
/// - `CODE STATE` (space-separated)
fn parse_oauth_callback_input(input: &str) -> Result<(String, String)> {
    let input = input.trim();

    // Try parsing as a URL first
    if input.starts_with("http://") || input.starts_with("https://") {
        if let Ok(url) = url::Url::parse(input) {
            let params: std::collections::HashMap<_, _> = url.query_pairs().collect();
            if let (Some(code), Some(state)) = (params.get("code"), params.get("state")) {
                return Ok((code.to_string(), state.to_string()));
            }
        }
        return Err(clankers::error::Error::ProviderAuth {
            message: "URL missing 'code' and/or 'state' query parameters.".to_string(),
        });
    }

    // Try code#state format
    if let Some((code, state)) = input.split_once('#')
        && !code.is_empty()
        && !state.is_empty()
    {
        return Ok((code.to_string(), state.to_string()));
    }

    // Try space-separated
    if let Some((code, state)) = input.split_once(' ') {
        let code = code.trim();
        let state = state.trim();
        if !code.is_empty() && !state.is_empty() {
            return Ok((code.to_string(), state.to_string()));
        }
    }

    Err(clankers::error::Error::ProviderAuth {
        message: format!(
            "Invalid code format: '{}'. Expected one of:\n  \
             code#state\n  \
             https://...?code=CODE&state=STATE",
            if input.len() > 40 { &input[..40] } else { input }
        ),
    })
}

#[snafu::report]
#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Determine if we're in headless mode (non-interactive output)
    let is_headless = cli.print.is_some() || cli.stdin || !matches!(cli.mode, OutputMode::Interactive);

    // Set up logging — headless mode defaults to WARN to avoid polluting
    // stdout/stderr with INFO-level noise.
    let log_level = if let Some(ref level) = cli.log_level {
        level.parse().unwrap_or(tracing::Level::INFO)
    } else if cli.verbose {
        tracing::Level::DEBUG
    } else if is_headless {
        tracing::Level::WARN
    } else {
        tracing::Level::INFO
    };

    // Respect RUST_LOG if explicitly set; otherwise use our default log level.
    // Always silence iroh and its transitive dependencies (quinn, netwatch, etc.)
    // unless the user explicitly sets RUST_LOG.
    let env_filter = if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::EnvFilter::from_default_env()
    } else {
        tracing_subscriber::EnvFilter::new("")
            .add_directive(log_level.into())
            .add_directive("iroh=error".parse().expect("static directive"))
            .add_directive("iroh_base=error".parse().expect("static directive"))
            .add_directive("iroh_blobs=error".parse().expect("static directive"))
            .add_directive("iroh_io=error".parse().expect("static directive"))
            .add_directive("iroh_metrics=error".parse().expect("static directive"))
            .add_directive("iroh_quinn=error".parse().expect("static directive"))
            .add_directive("iroh_quinn_proto=error".parse().expect("static directive"))
            .add_directive("iroh_quinn_udp=error".parse().expect("static directive"))
            .add_directive("iroh_relay=error".parse().expect("static directive"))
            .add_directive("iroh_tickets=error".parse().expect("static directive"))
            .add_directive("netwatch=error".parse().expect("static directive"))
            .add_directive("portmapper=error".parse().expect("static directive"))
            .add_directive("netlink_packet_route=error".parse().expect("static directive"))
            .add_directive("swarm_discovery=error".parse().expect("static directive"))
            .add_directive("wasmtime=error".parse().expect("static directive"))
            .add_directive("wasmtime_internal_cache=error".parse().expect("static directive"))
            .add_directive("extism=error".parse().expect("static directive"))
    };
    let subscriber = tracing_subscriber::fmt().with_env_filter(env_filter);

    if let Some(ref log_file) = cli.log_file {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)
            .expect("failed to open log file");
        subscriber.with_writer(file).init();
    } else {
        subscriber.with_writer(std::io::stderr).init();
    }

    info!("starting clankers");
    info!(?cli, "parsed CLI arguments");

    // Validate mutually exclusive options
    if cli.print.is_some() && cli.r#continue {
        return ConfigSnafu {
            message: "cannot use --print with --continue",
        }
        .fail();
    }

    if cli.resume.is_some() && cli.r#continue {
        return ConfigSnafu {
            message: "cannot use --resume with --continue",
        }
        .fail();
    }

    if cli.stream && cli.no_stream {
        return ConfigSnafu {
            message: "cannot use --stream with --no-stream",
        }
        .fail();
    }

    if (cli.zellij || cli.swarm) && cli.no_zellij {
        return ConfigSnafu {
            message: "cannot use --zellij/--swarm with --no-zellij",
        }
        .fail();
    }

    if cli.dry_run && cli.auto_approve {
        return ConfigSnafu {
            message: "cannot use --dry-run with --auto-approve",
        }
        .fail();
    }

    // Parse environment variables
    if !cli.env.is_empty() {
        for env_var in &cli.env {
            if let Some((key, value)) = env_var.split_once('=') {
                // SAFETY: We're setting environment variables early in main before any threads
                // are spawned, so this is safe.
                unsafe {
                    std::env::set_var(key, value);
                }
                info!("set environment variable: {}={}", key, value);
            } else {
                return ConfigSnafu {
                    message: format!("invalid environment variable format: {}", env_var),
                }
                .fail();
            }
        }
    }

    // Load direnv environment if an .envrc exists and the environment hasn't
    // already been loaded (e.g. when clankers is started from a daemon / RPC
    // context rather than a user shell with direnv hooks).
    let cwd = cli
        .cwd
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());
    clankers::util::direnv::load_direnv_if_needed(std::path::Path::new(&cwd));

    // Resolve paths and settings
    let paths = clankers::config::ClankersPaths::resolve();
    let project_paths = clankers::config::ProjectPaths::resolve(std::path::Path::new(&cwd));
    let settings = clankers::config::Settings::load_with_pi_fallback(
        paths.pi_settings.as_deref(),
        &paths.global_settings,
        &project_paths.settings,
    );

    let model = cli.model.clone().unwrap_or(settings.model.clone());

    // Build system prompt from multiple sources
    let base_prompt = cli
        .system_prompt
        .clone()
        .or_else(|| cli.system_prompt_file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()))
        .unwrap_or_else(|| clankers::agent::system_prompt::default_system_prompt().to_string());

    let resources = clankers::agent::system_prompt::discover_resources(&paths, &project_paths);
    let system_prompt = clankers::agent::system_prompt::assemble_system_prompt(
        &base_prompt,
        &resources,
        cli.system_prompt_prefix.as_deref().or(settings.system_prompt_prefix.as_deref()),
        cli.system_prompt_suffix.as_deref().or(settings.system_prompt_suffix.as_deref()),
    );

    match cli.command {
        Some(Commands::Version { verbose }) => {
            print!("clankers {}", env!("CARGO_PKG_VERSION"));
            if verbose {
                println!(" ({})", option_env!("CARGO_PKG_DESCRIPTION").unwrap_or("Rust terminal coding agent"));
            } else {
                println!();
            }
        }
        Some(Commands::Auth { action }) => match action {
            AuthAction::Login { account, code, .. } => {
                let account_name = account.as_deref().unwrap_or("default");

                // If code is provided directly, skip the browser flow
                let input = if let Some(code_input) = code {
                    code_input
                } else {
                    let (url, verifier_val) = clankers::provider::anthropic::oauth::build_auth_url();
                    println!("Logging in as account: {}", account_name);

                    // Try to auto-open the browser
                    if open::that_detached(&url).is_ok() {
                        println!("Opening browser automatically...\n");
                    } else {
                        println!("Could not open browser automatically.\n");
                    }

                    // Print a Ctrl+Clickable hyperlink using OSC 8
                    println!(
                        "Ctrl+Click or open this URL in your browser:\n\n  \x1b]8;;{}\x1b\\{}\x1b]8;;\x1b\\\n",
                        url, url
                    );
                    println!(
                        "After authorizing, paste the code or callback URL.\n\
                         Accepted formats:\n  \
                         code#state\n  \
                         https://...?code=CODE&state=STATE\n"
                    );

                    // Store verifier for use after reading input
                    // We need verifier in scope later, so save it in a file temporarily
                    let verifier_path = paths.global_config_dir.join(".login_verifier");
                    std::fs::create_dir_all(&paths.global_config_dir).ok();
                    std::fs::write(&verifier_path, &verifier_val).ok();

                    let mut buf = String::new();
                    std::io::stdin().read_line(&mut buf).expect("failed to read input");
                    buf.trim().to_string()
                };

                // Parse code#state from various input formats
                let (code_str, state_str) = parse_oauth_callback_input(&input)?;

                // Load or recover the PKCE verifier
                let verifier_path = paths.global_config_dir.join(".login_verifier");
                let verifier =
                    std::fs::read_to_string(&verifier_path).map_err(|_| clankers::error::Error::ProviderAuth {
                        message: "No login in progress. Run `clankers auth login` first to get the auth URL."
                            .to_string(),
                    })?;

                let creds =
                    clankers::provider::anthropic::oauth::exchange_code(&code_str, &state_str, &verifier).await?;

                // Clean up verifier file
                std::fs::remove_file(&verifier_path).ok();

                use clankers::provider::auth::AuthStoreExt;
                let mut store = clankers::provider::auth::AuthStore::load(&paths.global_auth);
                store.set_credentials(account_name, creds);
                store.switch_anthropic_account(account_name);
                store.save(&paths.global_auth)?;
                println!("Authentication successful! Credentials saved as '{}'.", account_name);
            }
            AuthAction::Status { .. } => {
                use clankers::provider::auth::AuthStoreExt;
                let store = clankers::provider::auth::AuthStore::load(&paths.global_auth);
                let accounts = store.list_anthropic_accounts();
                if !accounts.is_empty() {
                    println!("Accounts:");
                    for info in &accounts {
                        let marker = if info.is_active { "▸" } else { " " };
                        let status = if info.is_expired { "expired" } else { "valid" };
                        let label = info.label.as_ref().map(|l| format!(" ({})", l)).unwrap_or_default();
                        println!("  {} {}{} — {}", marker, info.name, label, status);
                    }
                } else if std::env::var("ANTHROPIC_API_KEY").is_ok() {
                    println!("Anthropic: API key set via ANTHROPIC_API_KEY");
                } else if let Some(ref pi_auth) = paths.pi_auth {
                    use clankers::provider::auth::AuthStoreExt;
                    let pi_store = clankers::provider::auth::AuthStore::load(pi_auth);
                    if !pi_store.list_anthropic_accounts().is_empty() {
                        println!("Using credentials from ~/.pi:");
                        for info in &pi_store.list_anthropic_accounts() {
                            let status = if info.is_expired { "expired" } else { "valid" };
                            println!("  {} — {}", info.name, status);
                        }
                    } else {
                        println!("Anthropic: not authenticated");
                    }
                } else {
                    println!("Anthropic: not authenticated");
                }
            }
            AuthAction::Logout { account, all, .. } => {
                use clankers::provider::auth::AuthStoreExt;
                let mut store = clankers::provider::auth::AuthStore::load(&paths.global_auth);
                if all {
                    // Remove all anthropic accounts
                    if let Some(prov) = store.providers.get_mut("anthropic") {
                        prov.accounts.clear();
                        prov.active_account = None;
                    }
                    store.save(&paths.global_auth)?;
                    println!("Removed all accounts.");
                } else {
                    let name = account.as_deref().unwrap_or(store.active_account_name());
                    let name = name.to_string();
                    if store.remove_anthropic_account(&name) {
                        store.save(&paths.global_auth)?;
                        println!("Removed account '{}'.", name);
                    } else {
                        eprintln!("No account '{}' found.", name);
                        std::process::exit(1);
                    }
                }
            }
            AuthAction::Switch { account } => {
                use clankers::provider::auth::AuthStoreExt;
                let mut store = clankers::provider::auth::AuthStore::load(&paths.global_auth);
                if store.switch_anthropic_account(&account) {
                    store.save(&paths.global_auth)?;
                    println!("Switched to account '{}'.", account);
                } else {
                    let names: Vec<_> = store
                        .providers
                        .get("anthropic")
                        .map(|p| p.accounts.keys().collect::<Vec<_>>())
                        .unwrap_or_default();
                    eprintln!("No account '{}'. Available: {:?}", account, names);
                    std::process::exit(1);
                }
            }
            AuthAction::Accounts => {
                use clankers::provider::auth::AuthStoreExt;
                let store = clankers::provider::auth::AuthStore::load(&paths.global_auth);
                print!("{}", store.account_summary());
            }
            _ => {
                eprintln!("This auth command is not yet implemented.");
                std::process::exit(1);
            }
        },
        Some(Commands::Config { action }) => match action {
            ConfigAction::Show { .. } => {
                println!("{}", serde_json::to_string_pretty(&settings).unwrap_or_default());
            }
            ConfigAction::Paths => {
                println!("Global config:   {}", paths.global_config_dir.display());
                println!("Global settings: {}", paths.global_settings.display());
                println!("Global auth:     {}", paths.global_auth.display());
                println!("Global agents:   {}", paths.global_agents_dir.display());
                println!("Global sessions: {}", paths.global_sessions_dir.display());
                println!("Project root:    {}", project_paths.root.display());
                println!("Project config:  {}", project_paths.config_dir.display());
                if let Some(ref pi_dir) = paths.pi_config_dir {
                    println!(
                        "Pi fallback:     {} (settings: {}, auth: {})",
                        pi_dir.display(),
                        if paths.pi_settings.is_some() { "found" } else { "none" },
                        if paths.pi_auth.is_some() { "found" } else { "none" },
                    );
                }
            }
            ConfigAction::Edit { project } => {
                let path = if project {
                    &project_paths.settings
                } else {
                    &paths.global_settings
                };
                let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
                let _ = std::process::Command::new(&editor).arg(path).status();
            }
            _ => {
                eprintln!("This config command is not yet implemented.");
                std::process::exit(1);
            }
        },
        Some(Commands::Session { action }) => match action {
            SessionAction::List { limit, all } => {
                let files = if all {
                    clankers::session::store::list_all_sessions(&paths.global_sessions_dir)
                } else {
                    clankers::session::store::list_sessions(&paths.global_sessions_dir, &cwd)
                };
                if files.is_empty() {
                    println!("No sessions found.");
                } else {
                    for (i, path) in files.iter().take(limit).enumerate() {
                        if let Some(summary) = clankers::session::store::read_session_summary(path) {
                            let date = summary.created_at.format("%Y-%m-%d %H:%M");
                            let preview = summary.first_user_message.as_deref().unwrap_or("(empty)");
                            let cwd_info = if all {
                                format!(" [{}]", summary.cwd)
                            } else {
                                String::new()
                            };
                            println!(
                                "  {}. {} | {} | {} msgs | {}{}\n     {}",
                                i + 1,
                                &summary.session_id[..8.min(summary.session_id.len())],
                                date,
                                summary.message_count,
                                summary.model,
                                cwd_info,
                                preview,
                            );
                        } else {
                            println!("  {}. {}", i + 1, path.display());
                        }
                    }
                    if files.len() > limit {
                        println!("\n  ({} more sessions)", files.len() - limit);
                    }
                }
            }
            SessionAction::Show { session_id, full } => {
                let found = clankers::session::store::find_session_by_id(&paths.global_sessions_dir, &cwd, &session_id);
                if let Some(path) = found {
                    if full {
                        // Dump raw JSONL
                        let content = std::fs::read_to_string(&path).unwrap_or_default();
                        println!("{}", content);
                    } else {
                        // Human-readable text format
                        match clankers::session::store::export_text(&path) {
                            Ok(text) => print!("{}", text),
                            Err(e) => eprintln!("Failed to read session: {}", e),
                        }
                    }
                } else {
                    eprintln!("Session not found: {}", session_id);
                    std::process::exit(1);
                }
            }
            SessionAction::Delete { session_id, .. } => {
                let found = clankers::session::store::find_session_by_id(&paths.global_sessions_dir, &cwd, &session_id);
                if let Some(path) = found {
                    std::fs::remove_file(&path).expect("failed to delete session");
                    println!("Session deleted.");
                } else {
                    eprintln!("Session not found: {}", session_id);
                    std::process::exit(1);
                }
            }
            SessionAction::DeleteAll { force } => {
                if !force {
                    eprintln!("This will delete ALL sessions for the current directory.");
                    eprintln!("Use --force to confirm.");
                    std::process::exit(1);
                }
                match clankers::session::store::purge_sessions(&paths.global_sessions_dir, &cwd) {
                    Ok(count) => println!("Deleted {} session(s).", count),
                    Err(e) => {
                        eprintln!("Failed to purge sessions: {}", e);
                        std::process::exit(1);
                    }
                }
            }
            SessionAction::Export {
                session_id,
                output,
                format,
            } => {
                let found = clankers::session::store::find_session_by_id(&paths.global_sessions_dir, &cwd, &session_id);
                if let Some(path) = found {
                    let result = match format {
                        ExportFormat::Json => clankers::session::store::export_json(&path),
                        ExportFormat::Markdown => clankers::session::store::export_markdown(&path),
                        ExportFormat::Text => clankers::session::store::export_text(&path),
                    };
                    match result {
                        Ok(content) => {
                            if let Some(ref out_path) = output {
                                let out = std::path::Path::new(out_path);
                                // If the path is just a filename, place it in .clankers/exports/
                                let resolved = if out.parent().is_none_or(|p| p.as_os_str().is_empty()) {
                                    let cwd_path = std::path::Path::new(&cwd);
                                    let exports_dir = cwd_path.join(".clankers").join("exports");
                                    if let Err(e) = std::fs::create_dir_all(&exports_dir) {
                                        eprintln!("Failed to create .clankers/exports: {}", e);
                                        std::process::exit(1);
                                    }
                                    clankers::util::fs::ensure_gitignore_entry(cwd_path, ".clankers/exports");
                                    exports_dir.join(out)
                                } else {
                                    out.to_path_buf()
                                };
                                match std::fs::write(&resolved, &content) {
                                    Ok(()) => println!("Exported to {}", resolved.display()),
                                    Err(e) => {
                                        eprintln!("Failed to write {}: {}", resolved.display(), e);
                                        std::process::exit(1);
                                    }
                                }
                            } else {
                                print!("{}", content);
                            }
                        }
                        Err(e) => {
                            eprintln!("Failed to export session: {}", e);
                            std::process::exit(1);
                        }
                    }
                } else {
                    eprintln!("Session not found: {}", session_id);
                    std::process::exit(1);
                }
            }
            SessionAction::Import { file } => {
                let source = std::path::Path::new(&file);
                if !source.is_file() {
                    eprintln!("File not found: {}", file);
                    std::process::exit(1);
                }
                match clankers::session::store::import_session(&paths.global_sessions_dir, source) {
                    Ok(dest) => println!("Imported session to {}", dest.display()),
                    Err(e) => {
                        eprintln!("Failed to import session: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        },
        Some(Commands::Share { read_only }) => {
            let session_name = clankers::zellij::session_name().unwrap_or_else(|| {
                eprintln!("Not inside a Zellij session. Start clankers inside Zellij first, or use: clankers --zellij");
                std::process::exit(1);
            });
            println!("Sharing Zellij session: {}", session_name);
            let secret_key = iroh::SecretKey::generate(&mut rand::rng());
            let node_id = secret_key.public();
            match clankers::zellij::streaming::host::host_session(&session_name, secret_key, read_only).await {
                Ok((_endpoint, psk)) => {
                    let psk_hex = clankers::zellij::streaming::handshake::psk_to_hex(&psk);
                    println!("\nSession shared! Give the remote user these credentials:\n");
                    println!("  clankers join {} {}\n", node_id, psk_hex);
                    println!("Press Ctrl+C to stop sharing.");
                    // Keep alive until interrupted
                    tokio::signal::ctrl_c().await.ok();
                    println!("\nStopped sharing.");
                }
                Err(e) => {
                    eprintln!("Failed to share session: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Join { node_id, psk }) => {
            let node_id: iroh::EndpointId = node_id.parse().unwrap_or_else(|e| {
                eprintln!("Invalid node ID: {}", e);
                std::process::exit(1);
            });
            let psk_bytes = clankers::zellij::streaming::handshake::psk_from_hex(&psk).unwrap_or_else(|| {
                eprintln!("Invalid PSK (expected 64-char hex string)");
                std::process::exit(1);
            });
            match clankers::zellij::streaming::guest::join_session(node_id, &psk_bytes).await {
                Ok(info) => {
                    println!("Connected to session: {}", info.session_name);
                    println!("Read-only: {}", info.read_only);
                    println!("\nRun this to attach:\n  zellij attach {}-remote\n", info.session_name);
                    println!("Press Ctrl+C to disconnect.");
                    tokio::signal::ctrl_c().await.ok();
                    println!("\nDisconnected.");
                }
                Err(e) => {
                    eprintln!("Failed to join session: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Rpc { identity, action }) => {
            let identity_path = identity
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| clankers::modes::rpc::iroh::identity_path(&paths));
            let identity = clankers::modes::rpc::iroh::Identity::load_or_generate(&identity_path);

            match action {
                RpcAction::Id => {
                    let pk = identity.public_key();
                    println!("Node ID: {}", pk);
                    println!("Short:   {}", pk.fmt_short());
                }
                RpcAction::Start {
                    with_agent,
                    tags,
                    allow_all,
                    heartbeat,
                    heartbeat_interval,
                } => {
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let addr = endpoint.addr();
                    println!("RPC server started");
                    println!("Node ID: {}", endpoint.id());
                    println!("Addr:    {:?}", addr);

                    // Build ACL
                    let acl_path = clankers::modes::rpc::iroh::allowlist_path(&paths);
                    let acl = if allow_all {
                        clankers::modes::rpc::iroh::AccessControl::open()
                    } else {
                        let allowed = clankers::modes::rpc::iroh::load_allowlist(&acl_path);
                        if allowed.is_empty() {
                            println!("WARNING: allowlist is empty — no peers can connect.");
                            println!("  Use --allow-all, or add peers with: clankers rpc allow <node-id>");
                        }
                        clankers::modes::rpc::iroh::AccessControl::from_allowlist(allowed)
                    };
                    println!(
                        "Auth: {}",
                        if acl.allow_all {
                            "open (--allow-all)".to_string()
                        } else {
                            format!("{} allowed peer(s)", acl.allowed.len())
                        }
                    );

                    // Discover available agent definitions
                    let project_paths = clankers::config::ProjectPaths::resolve(std::path::Path::new(&cwd));
                    let agent_scope = clankers::agents::definition::AgentScope::default();
                    let agent_registry = clankers::agents::discovery::discover_agents(
                        &paths.global_agents_dir,
                        Some(&project_paths.agents_dir),
                        &agent_scope,
                    );
                    let agent_names: Vec<String> = agent_registry.list().iter().map(|a| a.name.clone()).collect();

                    let agent_ctx = if with_agent {
                        let provider = clankers::modes::common::build_router(
                            cli.api_key.as_deref(),
                            cli.api_base.clone(),
                            &paths.global_auth,
                            paths.pi_auth.as_deref(),
                            cli.account.as_deref(),
                        )?;
                        let tools = clankers::modes::common::build_default_tools();
                        Some(clankers::modes::rpc::iroh::RpcContext {
                            provider,
                            tools,
                            settings: settings.clone(),
                            model: model.clone(),
                            system_prompt: system_prompt.clone(),
                        })
                    } else {
                        None
                    };

                    let receive_dir = paths.global_config_dir.join("received");
                    let state = std::sync::Arc::new(clankers::modes::rpc::iroh::ServerState {
                        meta: clankers::modes::rpc::iroh::NodeMeta {
                            tags: tags.clone(),
                            agent_names,
                        },
                        agent: agent_ctx,
                        acl,
                        receive_dir: Some(receive_dir),
                    });

                    println!("Agent support: {}", if state.agent.is_some() { "enabled" } else { "disabled" });
                    if !tags.is_empty() {
                        println!("Tags: {}", tags.join(", "));
                    }

                    // Start background heartbeat if requested
                    let cancel = tokio_util::sync::CancellationToken::new();
                    if heartbeat {
                        let registry_path = clankers::modes::rpc::peers::registry_path(&paths);
                        let interval = std::time::Duration::from_secs(heartbeat_interval);
                        let ep = std::sync::Arc::new(clankers::modes::rpc::iroh::start_endpoint(&identity).await?);
                        println!("Heartbeat: every {}s", heartbeat_interval);
                        tokio::spawn(clankers::modes::rpc::iroh::run_heartbeat(
                            ep,
                            registry_path,
                            interval,
                            cancel.clone(),
                        ));
                    }

                    println!("\nListening... (Ctrl+C to stop)\n");
                    println!("Test with:  clankers rpc ping {}", endpoint.id());

                    clankers::modes::rpc::iroh::serve_rpc(endpoint, state).await?;
                    cancel.cancel(); // Stop heartbeat on shutdown
                }
                RpcAction::Ping { node_id } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let request = clankers::modes::rpc::protocol::Request::new("ping", serde_json::json!({}));
                    println!("Pinging {}...", remote.fmt_short());
                    let start = std::time::Instant::now();
                    let response = clankers::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
                    let elapsed = start.elapsed();
                    if let Some(result) = response.ok {
                        println!("Response: {} ({}ms)", result, elapsed.as_millis());
                    } else if let Some(err) = response.error {
                        eprintln!("Error: {}", err);
                        std::process::exit(1);
                    }
                }
                RpcAction::Version { node_id } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let request = clankers::modes::rpc::protocol::Request::new("version", serde_json::json!({}));
                    let response = clankers::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
                    if let Some(result) = response.ok {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
                    } else if let Some(err) = response.error {
                        eprintln!("Error: {}", err);
                        std::process::exit(1);
                    }
                }
                RpcAction::Status { node_id } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let request = clankers::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
                    let response = clankers::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await?;
                    if let Some(result) = response.ok {
                        println!("{}", serde_json::to_string_pretty(&result).unwrap_or_default());
                    } else if let Some(err) = response.error {
                        eprintln!("Error: {}", err);
                        std::process::exit(1);
                    }
                }
                RpcAction::Prompt { node_id, text } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let request =
                        clankers::modes::rpc::protocol::Request::new("prompt", serde_json::json!({ "text": text }));
                    eprintln!("Sending prompt to {}...", remote.fmt_short());

                    // Use streaming RPC — print text deltas as they arrive
                    let (_notifications, response) =
                        clankers::modes::rpc::iroh::send_rpc_streaming(&endpoint, remote, &request, |notification| {
                            if let Some(method) = notification.get("method").and_then(|v| v.as_str()) {
                                match method {
                                    "agent.text_delta" => {
                                        if let Some(text) = notification
                                            .get("params")
                                            .and_then(|p| p.get("text"))
                                            .and_then(|v| v.as_str())
                                        {
                                            print!("{}", text);
                                            use std::io::Write;
                                            let _ = std::io::stdout().flush();
                                        }
                                    }
                                    "agent.tool_call" => {
                                        if let Some(params) = notification.get("params") {
                                            let tool = params.get("tool_name").and_then(|v| v.as_str()).unwrap_or("?");
                                            eprintln!("\n[tool: {}]", tool);
                                        }
                                    }
                                    "agent.tool_result" => {
                                        if let Some(params) = notification.get("params") {
                                            let is_error =
                                                params.get("is_error").and_then(|v| v.as_bool()).unwrap_or(false);
                                            if is_error {
                                                eprintln!("[tool error]");
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        })
                        .await?;

                    println!(); // newline after streamed text
                    if let Some(err) = response.error {
                        eprintln!("Error: {}", err);
                        std::process::exit(1);
                    }
                }
                RpcAction::Peers { action: peer_action } => {
                    let registry_path = clankers::modes::rpc::peers::registry_path(&paths);
                    let mut registry = clankers::modes::rpc::peers::PeerRegistry::load(&registry_path);

                    match peer_action {
                        PeerAction::List => {
                            let peers = registry.list();
                            if peers.is_empty() {
                                println!("No known peers. Add one with: clankers rpc peers add <node-id> <name>");
                            } else {
                                for peer in peers {
                                    let seen = peer
                                        .last_seen
                                        .map(|t| t.format("%Y-%m-%d %H:%M").to_string())
                                        .unwrap_or_else(|| "never".to_string());
                                    let caps = if peer.capabilities.accepts_prompts {
                                        "✓ prompts"
                                    } else {
                                        "✗ prompts"
                                    };
                                    let tags = if peer.capabilities.tags.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" [{}]", peer.capabilities.tags.join(", "))
                                    };
                                    let agents = if peer.capabilities.agents.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" agents: {}", peer.capabilities.agents.join(", "))
                                    };
                                    println!(
                                        "  {} ({}) — {} | last seen: {}{}{}",
                                        peer.name,
                                        &peer.node_id[..12.min(peer.node_id.len())],
                                        caps,
                                        seen,
                                        tags,
                                        agents,
                                    );
                                }
                            }
                        }
                        PeerAction::Add { node_id, name } => {
                            // Validate node_id format
                            let _: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                                eprintln!("Invalid node ID: {}", e);
                                std::process::exit(1);
                            });
                            registry.add(&node_id, &name);
                            registry.save(&registry_path).map_err(|e| clankers::error::Error::Io { source: e })?;
                            println!("Added peer '{}' ({})", name, &node_id[..12.min(node_id.len())]);
                        }
                        PeerAction::Remove { peer } => {
                            // Try as node_id first, then as name
                            let removed = if registry.remove(&peer) {
                                true
                            } else {
                                // Search by name
                                let found = registry.peers.values().find(|p| p.name == peer).map(|p| p.node_id.clone());
                                if let Some(nid) = found {
                                    registry.remove(&nid)
                                } else {
                                    false
                                }
                            };
                            if removed {
                                registry.save(&registry_path).map_err(|e| clankers::error::Error::Io { source: e })?;
                                println!("Removed peer '{}'", peer);
                            } else {
                                eprintln!("Peer '{}' not found", peer);
                                std::process::exit(1);
                            }
                        }
                        PeerAction::Probe { peer } => {
                            let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;

                            let targets: Vec<(String, String)> = if peer == "all" {
                                registry.list().iter().map(|p| (p.node_id.clone(), p.name.clone())).collect()
                            } else {
                                // Find by node_id or name
                                let node_id = if let Some(_p) = registry.peers.get(&peer) {
                                    peer.clone()
                                } else if let Some(p) = registry.peers.values().find(|p| p.name == peer) {
                                    p.node_id.clone()
                                } else {
                                    // Treat as raw node_id not in registry
                                    peer.clone()
                                };
                                let name = registry
                                    .peers
                                    .get(&node_id)
                                    .map(|p| p.name.clone())
                                    .unwrap_or_else(|| node_id[..12.min(node_id.len())].to_string());
                                vec![(node_id, name)]
                            };

                            for (node_id, name) in &targets {
                                let remote: iroh::PublicKey = match node_id.parse() {
                                    Ok(pk) => pk,
                                    Err(e) => {
                                        eprintln!("  {} — invalid node ID: {}", name, e);
                                        continue;
                                    }
                                };
                                print!("  Probing {}... ", name);
                                let request =
                                    clankers::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
                                match clankers::modes::rpc::iroh::send_rpc(&endpoint, remote, &request).await {
                                    Ok(response) => {
                                        if let Some(result) = response.ok {
                                            let caps = clankers::modes::rpc::peers::PeerCapabilities {
                                                accepts_prompts: result
                                                    .get("accepts_prompts")
                                                    .and_then(|v| v.as_bool())
                                                    .unwrap_or(false),
                                                agents: result
                                                    .get("agents")
                                                    .and_then(|v| v.as_array())
                                                    .map(|a| {
                                                        a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                                    })
                                                    .unwrap_or_default(),
                                                tools: result
                                                    .get("tools")
                                                    .and_then(|v| v.as_array())
                                                    .map(|a| {
                                                        a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                                    })
                                                    .unwrap_or_default(),
                                                tags: result
                                                    .get("tags")
                                                    .and_then(|v| v.as_array())
                                                    .map(|a| {
                                                        a.iter().filter_map(|v| v.as_str().map(String::from)).collect()
                                                    })
                                                    .unwrap_or_default(),
                                                version: result
                                                    .get("version")
                                                    .and_then(|v| v.as_str())
                                                    .map(String::from),
                                            };
                                            let prompt_status = if caps.accepts_prompts { "✓" } else { "✗" };
                                            println!(
                                                "online {} prompts | {} tools | tags: [{}]",
                                                prompt_status,
                                                caps.tools.len(),
                                                caps.tags.join(", "),
                                            );
                                            registry.update_capabilities(node_id, caps);
                                        } else {
                                            println!("online (no status data)");
                                            registry.touch(node_id);
                                        }
                                    }
                                    Err(e) => {
                                        println!("offline ({})", e);
                                    }
                                }
                            }
                            registry.save(&registry_path).map_err(|e| clankers::error::Error::Io { source: e })?;
                        }
                    }
                }
                RpcAction::Allow { node_id } => {
                    let _: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let acl_path = clankers::modes::rpc::iroh::allowlist_path(&paths);
                    let mut allowed = clankers::modes::rpc::iroh::load_allowlist(&acl_path);
                    allowed.insert(node_id.clone());
                    clankers::modes::rpc::iroh::save_allowlist(&acl_path, &allowed)
                        .map_err(|e| clankers::error::Error::Io { source: e })?;
                    println!("Allowed peer: {}", &node_id[..12.min(node_id.len())]);
                    println!("Total allowed: {}", allowed.len());
                }
                RpcAction::Deny { node_id } => {
                    let acl_path = clankers::modes::rpc::iroh::allowlist_path(&paths);
                    let mut allowed = clankers::modes::rpc::iroh::load_allowlist(&acl_path);
                    if allowed.remove(&node_id) {
                        clankers::modes::rpc::iroh::save_allowlist(&acl_path, &allowed)
                            .map_err(|e| clankers::error::Error::Io { source: e })?;
                        println!("Denied peer: {}", &node_id[..12.min(node_id.len())]);
                    } else {
                        eprintln!("Peer not in allowlist");
                        std::process::exit(1);
                    }
                }
                RpcAction::Allowed => {
                    let acl_path = clankers::modes::rpc::iroh::allowlist_path(&paths);
                    let allowed = clankers::modes::rpc::iroh::load_allowlist(&acl_path);
                    if allowed.is_empty() {
                        println!("No peers in allowlist. Use: clankers rpc allow <node-id>");
                        println!("Or start server with --allow-all");
                    } else {
                        println!("Allowed peers ({}):", allowed.len());
                        for nid in &allowed {
                            println!("  {}", nid);
                        }
                    }
                }
                RpcAction::Discover { mdns, scan_secs } => {
                    let registry_path = clankers::modes::rpc::peers::registry_path(&paths);
                    let mut registry = clankers::modes::rpc::peers::PeerRegistry::load(&registry_path);

                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;

                    // mDNS LAN scan — discover new peers automatically
                    if mdns {
                        let duration = std::time::Duration::from_secs(scan_secs);
                        let discovered = clankers::modes::rpc::iroh::discover_mdns_peers(&endpoint, duration).await;

                        if discovered.is_empty() {
                            println!("No new peers found via mDNS.");
                        } else {
                            println!("mDNS discovered {} peer(s):", discovered.len());
                            for (eid, _addr) in &discovered {
                                let nid = eid.to_string();
                                let short = &nid[..12.min(nid.len())];
                                if !registry.peers.contains_key(&nid) {
                                    registry.add(&nid, &format!("mdns-{}", short));
                                    println!("  + {} (auto-added as mdns-{})", short, short);
                                } else {
                                    println!("  = {} (already known)", short);
                                }
                            }
                            registry.save(&registry_path).map_err(|e| clankers::error::Error::Io { source: e })?;
                            println!();
                        }
                    }

                    let peers = registry.list().iter().map(|p| (p.node_id.clone(), p.name.clone())).collect::<Vec<_>>();

                    if peers.is_empty() {
                        println!("No known peers. Add some with: clankers rpc peers add <node-id> <name>");
                        println!("Or use --mdns to scan the local network.");
                        std::process::exit(0);
                    }

                    println!("Probing {} peer(s)...\n", peers.len());

                    let mut online = 0;
                    for (node_id, name) in &peers {
                        let remote: iroh::PublicKey = match node_id.parse() {
                            Ok(pk) => pk,
                            Err(_) => {
                                println!("  {} — invalid node ID", name);
                                continue;
                            }
                        };
                        let request = clankers::modes::rpc::protocol::Request::new("status", serde_json::json!({}));
                        match tokio::time::timeout(
                            std::time::Duration::from_secs(10),
                            clankers::modes::rpc::iroh::send_rpc(&endpoint, remote, &request),
                        )
                        .await
                        {
                            Ok(Ok(response)) => {
                                online += 1;
                                if let Some(result) = response.ok {
                                    let prompts =
                                        result.get("accepts_prompts").and_then(|v| v.as_bool()).unwrap_or(false);
                                    let tags: Vec<String> = result
                                        .get("tags")
                                        .and_then(|v| v.as_array())
                                        .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                        .unwrap_or_default();
                                    let prompt_icon = if prompts { "✓" } else { "✗" };
                                    let tags_str = if tags.is_empty() {
                                        String::new()
                                    } else {
                                        format!(" [{}]", tags.join(", "))
                                    };
                                    println!(
                                        "  ● {} ({}) — {} prompts{}",
                                        name,
                                        &node_id[..12.min(node_id.len())],
                                        prompt_icon,
                                        tags_str
                                    );

                                    // Update registry
                                    let caps = clankers::modes::rpc::peers::PeerCapabilities {
                                        accepts_prompts: prompts,
                                        agents: result
                                            .get("agents")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                        tools: result
                                            .get("tools")
                                            .and_then(|v| v.as_array())
                                            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                                            .unwrap_or_default(),
                                        tags,
                                        version: result.get("version").and_then(|v| v.as_str()).map(String::from),
                                    };
                                    registry.update_capabilities(node_id, caps);
                                } else {
                                    println!("  ● {} — online", name);
                                    registry.touch(node_id);
                                }
                            }
                            Ok(Err(_)) | Err(_) => {
                                println!("  ○ {} ({}) — offline", name, &node_id[..12.min(node_id.len())]);
                            }
                        }
                    }
                    registry.save(&registry_path).map_err(|e| clankers::error::Error::Io { source: e })?;
                    println!("\n{}/{} peers online", online, peers.len());
                }
                RpcAction::SendFile { node_id, file } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let file_path = std::path::Path::new(&file);
                    if !file_path.exists() {
                        eprintln!("File not found: {}", file);
                        std::process::exit(1);
                    }
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    let file_size = std::fs::metadata(file_path).map(|m| m.len()).unwrap_or(0);
                    println!("Sending '{}' ({} bytes) to {}...", file_path.display(), file_size, remote.fmt_short());
                    let response = clankers::modes::rpc::iroh::send_file(&endpoint, remote, file_path).await?;
                    if let Some(result) = response.ok {
                        let remote_path = result.get("path").and_then(|v| v.as_str()).unwrap_or("?");
                        let size = result.get("size").and_then(|v| v.as_u64()).unwrap_or(0);
                        println!("✓ Sent {} bytes → {}", size, remote_path);
                    } else if let Some(err) = response.error {
                        eprintln!("Error: {}", err);
                        std::process::exit(1);
                    }
                }
                RpcAction::RecvFile {
                    node_id,
                    remote_path,
                    output,
                } => {
                    let remote: iroh::PublicKey = node_id.parse().unwrap_or_else(|e| {
                        eprintln!("Invalid node ID: {}", e);
                        std::process::exit(1);
                    });
                    let local_path = output.map(std::path::PathBuf::from).unwrap_or_else(|| {
                        let name = std::path::Path::new(&remote_path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("downloaded");
                        std::path::PathBuf::from(name)
                    });
                    let endpoint = clankers::modes::rpc::iroh::start_endpoint(&identity).await?;
                    println!("Downloading '{}' from {} → {}...", remote_path, remote.fmt_short(), local_path.display());
                    let total =
                        clankers::modes::rpc::iroh::recv_file(&endpoint, remote, &remote_path, &local_path).await?;
                    println!("✓ Received {} bytes → {}", total, local_path.display());
                }
            }
        }
        Some(Commands::Daemon {
            tags,
            allow_all,
            matrix,
            heartbeat,
            max_sessions,
        }) => {
            let provider = clankers::modes::common::build_router(
                cli.api_key.as_deref(),
                cli.api_base.clone(),
                &paths.global_auth,
                paths.pi_auth.as_deref(),
                cli.account.as_deref(),
            )?;

            let tools = clankers::modes::common::build_default_tools();

            let config = clankers::modes::daemon::DaemonConfig {
                model: model.clone(),
                system_prompt: system_prompt.clone(),
                settings: settings.clone(),
                tags,
                allow_all,
                enable_matrix: matrix,
                heartbeat_secs: heartbeat,
                max_sessions,
            };

            clankers::modes::daemon::run_daemon(provider, tools, config, &paths).await?;
        }
        Some(Commands::MergeDaemon { interval, once }) => {
            let repo_root = std::path::PathBuf::from(&cwd);

            // Try to build a provider for LLM conflict resolution
            let provider = clankers::modes::common::build_router(
                cli.api_key.as_deref(),
                cli.api_base.clone(),
                &paths.global_auth,
                paths.pi_auth.as_deref(),
                None,
            )
            .ok();

            let db_path = paths.global_config_dir.join("clankers.db");
            let db = clankers::db::Db::open(&db_path).expect("failed to open database");
            clankers::worktree::merge_daemon::run_polling(db, repo_root, interval, once, provider, model).await;
        }
        Some(Commands::Plugin { action }) => {
            let project_paths = clankers::config::ProjectPaths::resolve(std::path::Path::new(&cwd));
            let plugin_manager = clankers::modes::common::init_plugin_manager(
                &paths.global_plugins_dir,
                Some(&project_paths.plugins_dir),
                &[&project_paths.plugins_root_dir],
            );
            match action {
                PluginAction::List { verbose } => {
                    let mgr = plugin_manager.lock().unwrap_or_else(|e| e.into_inner());
                    let plugins = mgr.list();
                    if plugins.is_empty() {
                        println!("No plugins found.");
                        println!("\nPlugin directories:");
                        println!("  Global:  {}", paths.global_plugins_dir.display());
                        println!("  Project: {}", project_paths.plugins_dir.display());
                    } else {
                        for p in plugins {
                            if verbose {
                                println!(
                                    "{} v{} [{:?}]\n  {}\n  Path: {}\n  Tools: {}\n  Commands: {}\n  Events: {}\n  Permissions: {}",
                                    p.name,
                                    p.version,
                                    p.state,
                                    p.manifest.description,
                                    p.path.display(),
                                    p.manifest.tools.join(", "),
                                    p.manifest.commands.join(", "),
                                    p.manifest.events.join(", "),
                                    p.manifest.permissions.join(", "),
                                );
                            } else {
                                let state = match &p.state {
                                    clankers::plugin::PluginState::Active => "✓",
                                    clankers::plugin::PluginState::Loaded => "○",
                                    clankers::plugin::PluginState::Error(_) => "✗",
                                    clankers::plugin::PluginState::Disabled => "−",
                                };
                                println!("{} {} v{} — {}", state, p.name, p.version, p.manifest.description);
                            }
                        }
                    }
                }
                PluginAction::Show { name } => {
                    let mgr = plugin_manager.lock().unwrap_or_else(|e| e.into_inner());
                    if let Some(p) = mgr.get(&name) {
                        println!("Name:        {}", p.name);
                        println!("Version:     {}", p.version);
                        println!("State:       {:?}", p.state);
                        println!("Description: {}", p.manifest.description);
                        println!("Path:        {}", p.path.display());
                        println!("WASM:        {}", p.manifest.wasm.as_deref().unwrap_or("plugin.wasm"));
                        println!("Kind:        {:?}", p.manifest.kind);
                        println!(
                            "Tools:       {}",
                            if p.manifest.tools.is_empty() {
                                "(none)".to_string()
                            } else {
                                p.manifest.tools.join(", ")
                            }
                        );
                        println!(
                            "Commands:    {}",
                            if p.manifest.commands.is_empty() {
                                "(none)".to_string()
                            } else {
                                p.manifest.commands.join(", ")
                            }
                        );
                        println!(
                            "Events:      {}",
                            if p.manifest.events.is_empty() {
                                "(none)".to_string()
                            } else {
                                p.manifest.events.join(", ")
                            }
                        );
                        println!(
                            "Permissions: {}",
                            if p.manifest.permissions.is_empty() {
                                "(none)".to_string()
                            } else {
                                p.manifest.permissions.join(", ")
                            }
                        );
                        if !p.manifest.tool_definitions.is_empty() {
                            println!("\nTool definitions:");
                            for td in &p.manifest.tool_definitions {
                                println!("  {} — {}", td.name, td.description);
                                println!("    Handler: {}", td.handler);
                                println!(
                                    "    Schema:  {}",
                                    serde_json::to_string(&td.input_schema).unwrap_or_default()
                                );
                            }
                        }
                    } else {
                        eprintln!("Plugin '{}' not found.", name);
                        std::process::exit(1);
                    }
                }
                PluginAction::Install { source, project } => {
                    let source_path = std::path::Path::new(&source);
                    let manifest_path = source_path.join("plugin.json");
                    if !manifest_path.is_file() {
                        eprintln!("No plugin.json found at: {}", manifest_path.display());
                        std::process::exit(1);
                    }
                    let manifest =
                        clankers::plugin::manifest::PluginManifest::load(&manifest_path).unwrap_or_else(|| {
                            eprintln!("Failed to parse plugin.json at: {}", manifest_path.display());
                            std::process::exit(1);
                        });
                    let dest_dir = if project {
                        project_paths.plugins_dir.join(&manifest.name)
                    } else {
                        paths.global_plugins_dir.join(&manifest.name)
                    };
                    if dest_dir.exists() {
                        eprintln!("Plugin '{}' already installed at: {}", manifest.name, dest_dir.display());
                        eprintln!("Remove it first with: clankers plugin uninstall {}", manifest.name);
                        std::process::exit(1);
                    }
                    // Copy plugin directory
                    std::fs::create_dir_all(&dest_dir).unwrap_or_else(|e| {
                        eprintln!("Failed to create directory {}: {}", dest_dir.display(), e);
                        std::process::exit(1);
                    });
                    let dir_entries = match std::fs::read_dir(source_path) {
                        Ok(entries) => entries,
                        Err(e) => {
                            eprintln!("Failed to read directory {}: {}", source_path.display(), e);
                            std::process::exit(1);
                        }
                    };
                    for entry in dir_entries.flatten() {
                        let src = entry.path();
                        if src.is_file() {
                            let dest = dest_dir.join(entry.file_name());
                            std::fs::copy(&src, &dest).unwrap_or_else(|e| {
                                eprintln!("Failed to copy {}: {}", src.display(), e);
                                std::process::exit(1);
                            });
                        }
                    }
                    let scope = if project { "project" } else { "global" };
                    println!("Installed plugin '{}' v{} to {} plugins.", manifest.name, manifest.version, scope);
                    println!("  Path: {}", dest_dir.display());
                }
                PluginAction::Uninstall { name, project } => {
                    let dest_dir = if project {
                        project_paths.plugins_dir.join(&name)
                    } else {
                        paths.global_plugins_dir.join(&name)
                    };
                    if !dest_dir.exists() {
                        eprintln!("Plugin '{}' not found at: {}", name, dest_dir.display());
                        std::process::exit(1);
                    }
                    std::fs::remove_dir_all(&dest_dir).unwrap_or_else(|e| {
                        eprintln!("Failed to remove plugin directory: {}", e);
                        std::process::exit(1);
                    });
                    println!("Uninstalled plugin '{}'.", name);
                }
            }
        }
        Some(_) => {
            eprintln!("This command is not yet implemented.");
            std::process::exit(1);
        }
        None => {
            // Main agent mode — determine if print, json, or interactive
            let prompt = if let Some(ref p) = cli.print {
                Some(p.clone())
            } else if cli.stdin {
                let mut input = String::new();
                std::io::Read::read_to_string(&mut std::io::stdin(), &mut input).expect("failed to read stdin");
                Some(input)
            } else {
                None
            };

            // If --agent is specified, look up the agent definition and override model/system_prompt
            let (model, system_prompt) = if let Some(ref agent_name) = cli.agent {
                let agent_scope = cli
                    .agent_scope
                    .as_ref()
                    .map(|s| match s {
                        AgentScopeArg::User => clankers::agents::definition::AgentScope::User,
                        AgentScopeArg::Project => clankers::agents::definition::AgentScope::Project,
                        AgentScopeArg::Both => clankers::agents::definition::AgentScope::Both,
                    })
                    .unwrap_or_default();

                let registry = clankers::agents::discovery::discover_agents(
                    &paths.global_agents_dir,
                    Some(&project_paths.agents_dir),
                    &agent_scope,
                );

                if let Some(agent_def) = registry.get(agent_name) {
                    let m = agent_def.model.clone().unwrap_or(model);
                    let sp = agent_def.system_prompt.clone();
                    (m, sp)
                } else {
                    eprintln!("Agent '{}' not found. Available agents:", agent_name);
                    for a in registry.list() {
                        eprintln!("  - {}: {}", a.name, a.description);
                    }
                    std::process::exit(1);
                }
            } else {
                (model, system_prompt)
            };

            // Initialize sandbox: path policy deny-list for all tools.
            // Landlock is applied per-bash-child, not to clankers itself.
            clankers::tools::sandbox::init_policy();

            // Initialize plugin manager for all agent modes
            let plugin_manager = clankers::modes::common::init_plugin_manager(
                &paths.global_plugins_dir,
                Some(&project_paths.plugins_dir),
                &[&project_paths.plugins_root_dir],
            );

            if let Some(prompt) = prompt {
                // Build provider and tools (including plugin tools)
                let provider = clankers::modes::common::build_router(
                    cli.api_key.as_deref(),
                    cli.api_base.clone(),
                    &paths.global_auth,
                    paths.pi_auth.as_deref(),
                    cli.account.as_deref(),
                )?;
                let tools = if cli.tools.as_deref() == Some("none") || cli.tools.as_deref() == Some("") {
                    Vec::new()
                } else {
                    let all_tools =
                        clankers::modes::common::build_all_tools(None, None, None, Some(&plugin_manager), None, None);
                    if let Some(ref allowed) = cli.tools {
                        let allowed_set: std::collections::HashSet<&str> =
                            allowed.split(',').map(|s| s.trim()).collect();
                        if allowed_set.contains("all") {
                            all_tools
                        } else {
                            all_tools
                                .into_iter()
                                .filter(|t| allowed_set.contains(t.definition().name.as_str()))
                                .collect()
                        }
                    } else {
                        all_tools
                    }
                };

                // Apply --attach: prepend file contents to the prompt
                let attach_context = clankers::modes::common::build_attach_context(&cli.attach);
                let full_prompt = if attach_context.is_empty() {
                    prompt
                } else {
                    format!("{}{}", attach_context, prompt)
                };

                // Apply --max-iterations to settings
                let mut settings = settings;
                // (max_iterations is stored on the CLI; we pass it through settings.max_tokens
                //  as a proxy — the actual iteration limit is in TurnConfig inside the agent)
                if let Some(max_tokens) = cli.max_tokens {
                    settings.max_tokens = max_tokens;
                }

                // Apply --thinking / --thinking-budget
                let thinking_config = if cli.thinking || cli.thinking_budget.is_some() {
                    Some(clankers::provider::ThinkingConfig {
                        enabled: true,
                        budget_tokens: cli.thinking_budget.or(Some(10_000)),
                    })
                } else {
                    None
                };

                match cli.mode {
                    OutputMode::Json => {
                        let json_opts = clankers::modes::json::JsonOptions {
                            output_file: cli.output.clone(),
                            thinking: thinking_config,
                        };
                        clankers::modes::json::run_json_with_options(
                            &full_prompt,
                            provider,
                            tools,
                            settings,
                            model,
                            system_prompt,
                            json_opts,
                        )
                        .await?;
                    }
                    OutputMode::Markdown => {
                        let print_opts = clankers::modes::print::PrintOptions {
                            output_file: cli.output.clone(),
                            show_stats: cli.stats,
                            show_tools: cli.verbose,
                            format: clankers::modes::print::PrintFormat::Markdown,
                            thinking: thinking_config,
                        };
                        clankers::modes::print::run_print_with_options(
                            &full_prompt,
                            provider,
                            tools,
                            settings,
                            model,
                            system_prompt,
                            print_opts,
                        )
                        .await?;
                    }
                    _ => {
                        // Print / Plain / Interactive-with-prompt all use text format
                        let print_opts = clankers::modes::print::PrintOptions {
                            output_file: cli.output.clone(),
                            show_stats: cli.stats,
                            show_tools: cli.verbose,
                            format: clankers::modes::print::PrintFormat::Text,
                            thinking: thinking_config,
                        };
                        clankers::modes::print::run_print_with_options(
                            &full_prompt,
                            provider,
                            tools,
                            settings,
                            model,
                            system_prompt,
                            print_opts,
                        )
                        .await?;
                    }
                }
            } else {
                // Interactive mode — try to use the router daemon (auto-starting if needed)
                let provider = clankers::modes::common::build_router_with_rpc(
                    cli.api_key.as_deref(),
                    cli.api_base.clone(),
                    &paths.global_auth,
                    paths.pi_auth.as_deref(),
                    cli.account.as_deref(),
                )
                .await?;
                let resume_opts = clankers::modes::interactive::ResumeOptions {
                    session_id: cli.resume.clone(),
                    continue_last: cli.r#continue,
                    no_session: cli.no_session,
                };
                // Register prompt templates for slash command completions
                let template_names: Vec<(String, String)> =
                    resources.prompts.iter().map(|p| (p.name.clone(), p.description.clone())).collect();
                clankers::slash_commands::register_prompt_templates(&template_names);

                clankers::modes::interactive::run_interactive(
                    provider,
                    settings,
                    model,
                    system_prompt,
                    cwd,
                    Some(plugin_manager),
                    resume_opts,
                )
                .await?;
            }
        }
    }

    Ok(())
}
