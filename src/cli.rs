//! CLI argument definitions (clap derive types).
//!
//! Separated from `main.rs` so that command handler modules can import
//! these types without creating circular dependencies.

use clap::Parser;
use clap::Subcommand;
use clap::ValueEnum;

#[derive(Parser, Debug)]
#[command(
    name = "clankers",
    about = "clankers — a Rust terminal coding agent",
    version,
    long_about = None,
)]
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "clap default_value_t uses unwrap in macro expansion"))]
pub struct Cli {
    /// Enable verbose logging
    #[arg(short, long)]
    pub verbose: bool,

    /// Print mode: execute a single prompt and exit
    #[arg(short, long, value_name = "PROMPT")]
    pub print: Option<String>,

    /// Output mode
    #[arg(long, value_enum, default_value_t = OutputMode::Interactive)]
    pub mode: OutputMode,

    /// Model to use (overrides settings)
    #[arg(long, value_name = "MODEL")]
    pub model: Option<String>,

    /// Provider to use (anthropic, openai, etc.)
    #[arg(long, value_name = "PROVIDER")]
    pub provider: Option<String>,

    /// Maximum output tokens
    #[arg(long, value_name = "TOKENS")]
    pub max_tokens: Option<usize>,

    /// Temperature (0.0-1.0)
    #[arg(long, value_name = "TEMP")]
    pub temperature: Option<f32>,

    /// Top-p sampling (0.0-1.0)
    #[arg(long, value_name = "P")]
    pub top_p: Option<f32>,

    /// Top-k sampling
    #[arg(long, value_name = "K")]
    pub top_k: Option<u32>,

    /// System prompt (overrides default)
    #[arg(long, value_name = "PROMPT")]
    pub system_prompt: Option<String>,

    /// Append to system prompt
    #[arg(long, value_name = "TEXT")]
    pub system_prompt_suffix: Option<String>,

    /// Prepend to system prompt
    #[arg(long, value_name = "TEXT")]
    pub system_prompt_prefix: Option<String>,

    /// Load system prompt from file
    #[arg(long, value_name = "FILE")]
    pub system_prompt_file: Option<String>,

    /// Tool mode: "all", "core", "none", tier names (core,specialty,orchestration,matrix),
    /// or comma-separated tool names for fine-grained control. Default: auto (mode-dependent).
    #[arg(long, value_name = "TOOLS")]
    pub tools: Option<String>,

    /// Attach files for context (can be specified multiple times)
    #[arg(long, value_name = "FILE")]
    pub attach: Vec<String>,

    /// Working directory
    #[arg(long, value_name = "DIR")]
    pub cwd: Option<String>,

    /// Resume a previous session by ID
    #[arg(long, value_name = "SESSION_ID")]
    pub resume: Option<String>,

    /// Continue the most recent session
    #[arg(long, short = 'c')]
    pub r#continue: bool,

    /// Disable git worktree isolation
    #[arg(long)]
    pub no_worktree: bool,

    /// (Deprecated) Zellij flags — pane management is now built into the TUI
    #[arg(long, hide = true)]
    pub zellij: bool,
    #[arg(long, hide = true)]
    pub swarm: bool,
    #[arg(long, hide = true)]
    pub no_zellij: bool,

    /// Disable session persistence
    #[arg(long)]
    pub no_session: bool,

    /// Force daemon mode (auto-start daemon + attach)
    #[arg(long)]
    pub daemon: bool,

    /// Force in-process mode (skip daemon, run agent directly)
    #[arg(long)]
    pub no_daemon: bool,

    /// Disable prompt caching
    #[arg(long)]
    pub no_cache: bool,

    /// Cache TTL for prompt caching ("5m" default, "1h" for 1-hour at 2× cost)
    #[arg(long, value_name = "TTL")]
    pub cache_ttl: Option<String>,

    /// Agent definition to use
    #[arg(long, value_name = "AGENT")]
    pub agent: Option<String>,

    /// Agent scope for discovery
    #[arg(long, value_enum)]
    pub agent_scope: Option<AgentScopeArg>,

    /// Enable extended thinking
    #[arg(long)]
    pub thinking: bool,

    /// Thinking budget tokens
    #[arg(long, value_name = "TOKENS")]
    pub thinking_budget: Option<usize>,

    /// Read prompt from stdin
    #[arg(long)]
    pub stdin: bool,

    /// Output file for print mode
    #[arg(short = 'o', long, value_name = "FILE")]
    pub output: Option<String>,

    /// Account name to use (for multi-account setups)
    #[arg(long, value_name = "NAME")]
    pub account: Option<String>,

    /// API key override (for testing)
    #[arg(long, value_name = "KEY", env = "CLANKERS_API_KEY")]
    pub api_key: Option<String>,

    /// API base URL override
    #[arg(long, value_name = "URL")]
    pub api_base: Option<String>,

    /// Request timeout in seconds
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<u64>,

    /// Maximum cost budget in USD
    #[arg(long, value_name = "DOLLARS", alias = "budget")]
    pub max_cost: Option<f64>,

    /// Enable automatic model routing by task complexity
    #[arg(long)]
    pub enable_routing: bool,

    /// Maximum loop iterations
    #[arg(long, value_name = "N", default_value_t = 25)]
    pub max_iterations: usize,

    /// Confirm before executing tool calls
    #[arg(long)]
    pub confirm: bool,

    /// Dry run: show tool calls without executing
    #[arg(long)]
    pub dry_run: bool,

    /// Enable auto-approval of tool calls
    #[arg(long)]
    pub auto_approve: bool,

    /// Load skill by name
    #[arg(long, value_name = "SKILL")]
    pub skill: Vec<String>,

    /// Skill directory path
    #[arg(long, value_name = "DIR")]
    pub skill_dir: Option<String>,

    /// Environment variables to set (KEY=VALUE)
    #[arg(long, value_name = "KEY=VALUE")]
    pub env: Vec<String>,

    /// Stream output in real-time
    #[arg(long)]
    pub stream: bool,

    /// Disable streaming
    #[arg(long)]
    pub no_stream: bool,

    /// Inline output mode (shorthand for --mode inline)
    #[arg(long)]
    pub inline: bool,

    /// Show token usage stats
    #[arg(long)]
    pub stats: bool,

    /// Log file path
    #[arg(long, value_name = "FILE")]
    pub log_file: Option<String>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, value_name = "LEVEL")]
    pub log_level: Option<String>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum OutputMode {
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
    /// Inline styled scrollback output (via rat-inline)
    Inline,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum AgentScopeArg {
    /// User-level agents only
    User,
    /// Project-level agents only
    Project,
    /// Both user and project agents
    Both,
}

#[derive(Subcommand, Debug)]
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "clap default_value_t uses unwrap in macro expansion"))]
pub enum Commands {
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
    #[cfg(feature = "zellij-share")]
    Share {
        /// Read-only mode (remote guests cannot type)
        #[arg(long)]
        read_only: bool,
    },
    /// Join a remote shared Zellij session
    #[cfg(feature = "zellij-share")]
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
    /// Run and manage the background daemon
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    /// Manage capability tokens (UCAN auth)
    Token {
        #[command(subcommand)]
        action: TokenAction,
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
    /// Attach TUI to a daemon session (reads events from socket instead of local agent)
    Attach {
        /// Session ID to attach to (omit to list sessions interactively)
        session_id: Option<String>,
        /// Create a new session if none specified
        #[arg(long)]
        new: bool,
        /// Model for new sessions (only with --new)
        #[arg(long)]
        model: Option<String>,
        /// Connect to a remote daemon via iroh QUIC (node ID or peer name)
        #[arg(long)]
        remote: Option<String>,
        /// Auto-start daemon if not running
        #[arg(long)]
        auto_daemon: bool,
        /// Create session with read-only capabilities (no write tools)
        #[arg(long)]
        read_only: bool,
        /// Comma-separated capability list for session scoping (e.g., "read,grep,bash")
        #[arg(long, value_delimiter = ',')]
        capabilities: Vec<String>,
    },
    /// List daemon sessions (shorthand for `daemon sessions`)
    Ps {
        /// Show all details including socket paths
        #[arg(short, long)]
        all: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum RpcAction {
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
pub enum PeerAction {
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
pub enum SessionAction {
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
    /// Migrate JSONL sessions to Automerge format
    Migrate {
        /// Session ID to migrate (or --all for all sessions)
        session_id: Option<String>,
        /// Migrate all JSONL sessions
        #[arg(long)]
        all: bool,
    },
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ExportFormat {
    Json,
    Markdown,
    Text,
}

#[derive(Subcommand, Debug)]
pub enum ConfigAction {
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
        /// Initialize as Nickel (.ncl) instead of JSON
        #[arg(long)]
        nickel: bool,
        /// Initialize global config instead of project
        #[arg(long)]
        global: bool,
    },
    /// Validate config files without starting a session
    Check,
    /// Export merged config as JSON
    Export {
        /// Export only global config (skip project merge)
        #[arg(long)]
        global: bool,
    },
}

#[derive(Subcommand, Debug)]
pub enum AuthAction {
    /// Authenticate with a provider (OAuth)
    #[command(long_about = "Start or complete provider OAuth login.\n\nIf --provider is omitted, clankers keeps the Anthropic default.\nUse --provider openai-codex for ChatGPT Plus or Pro personal subscriptions. openai-codex stays separate from API-key openai.\nUse --account <name> to reuse your existing local account names. Unsupported openai-codex plans stay authenticated but unavailable for Codex use.")]
    Login {
        /// Provider name (`anthropic`, `openai-codex`)
        #[arg(long)]
        provider: Option<String>,
        /// Account name (e.g. "work", "personal"). Defaults to "default".
        #[arg(long, value_name = "NAME")]
        account: Option<String>,
        /// Authorization code#state from the OAuth callback (skips interactive prompt)
        #[arg(long, value_name = "CODE")]
        code: Option<String>,
    },
    /// Show current auth status
    #[command(long_about = "Show provider-scoped auth status.\n\nWithout --provider or --all, clankers keeps the Anthropic default summary.\nUse --provider openai-codex or --all to inspect Codex subscription accounts, including entitled, authenticated-but-not-entitled, and entitlement-check-failed states. API-key openai remains a separate provider path.")]
    Status {
        /// Provider name (`anthropic`, `openai`, `openai-codex`, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Show for all providers
        #[arg(long)]
        all: bool,
    },
    /// Remove stored credentials
    Logout {
        /// Provider name (`anthropic`, `openai`, `openai-codex`, etc.)
        #[arg(long)]
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
        /// Provider name (`anthropic`, `openai-codex`, etc.)
        #[arg(long)]
        provider: Option<String>,
        /// Account name to switch to
        account: String,
    },
    /// List all accounts
    Accounts,
    /// Export one provider/account record as JSON
    Export {
        /// Provider name (`anthropic`, `openai-codex`, etc.)
        provider: String,
        /// Account name to export
        #[arg(long, value_name = "NAME")]
        account: Option<String>,
    },
    /// Import one provider/account record from JSON
    Import {
        /// Path to JSON record (`-` for stdin)
        #[arg(long, default_value = "-")]
        input: String,
    },
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
pub enum SkillAction {
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
pub enum AgentAction {
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
pub enum PluginAction {
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

#[derive(Subcommand, Debug)]
pub enum TokenAction {
    /// Create a new capability token
    Create {
        /// Comma-separated list of allowed tools (e.g., "read,grep,find") or "*" for all
        #[arg(long, value_name = "TOOLS")]
        tools: Option<String>,
        /// Shorthand for read-only access (read, grep, find, ls tools only)
        #[arg(long)]
        read_only: bool,
        /// Token lifetime (e.g., "1h", "24h", "7d", "30d", "365d")
        #[arg(long, value_name = "DURATION", default_value = "24h")]
        expire: String,
        /// Lock token to a specific public key (audience)
        #[arg(long, value_name = "PUBKEY")]
        r#for: Option<String>,
        /// Delegate from a parent token (base64-encoded)
        #[arg(long, value_name = "TOKEN")]
        from: Option<String>,
        /// Comma-separated list of allowed bot commands or "*" for all
        #[arg(long, value_name = "COMMANDS")]
        bot_commands: Option<String>,
        /// Allow session management (restart, compact)
        #[arg(long)]
        session_manage: bool,
        /// Allow model switching
        #[arg(long)]
        model_switch: bool,
        /// Allow delegation (creating child tokens)
        #[arg(long)]
        delegate: bool,
        /// File access prefix (e.g., "/home/user/project/")
        #[arg(long, value_name = "PREFIX")]
        file_prefix: Option<String>,
        /// File access is read-only (used with --file-prefix)
        #[arg(long)]
        file_read_only: bool,
        /// Shell command pattern (e.g., "pg_*" or "*")
        #[arg(long, value_name = "PATTERN")]
        shell: Option<String>,
        /// Working directory constraint for shell commands
        #[arg(long, value_name = "DIR")]
        shell_wd: Option<String>,
        /// Create a full-access root token (all capabilities)
        #[arg(long)]
        root: bool,
    },
    /// List issued tokens
    List,
    /// Revoke a token by its hash (hex-encoded)
    Revoke {
        /// Token hash (hex string) or base64 token to revoke
        hash: String,
    },
    /// Decode and display token details
    Info {
        /// Base64-encoded token
        token: String,
    },
}

#[derive(Subcommand, Debug)]
pub enum DaemonAction {
    /// Start the daemon (foreground by default, -d to background)
    Start {
        /// Run in the background (daemonize)
        #[arg(short = 'd', long)]
        background: bool,
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
    /// Stop the running daemon
    Stop,
    /// Show daemon status
    Status,
    /// List active sessions
    Sessions {
        /// Show all details including socket paths
        #[arg(short, long)]
        all: bool,
    },
    /// Create a new session on the daemon
    Create {
        /// Model to use (default: daemon's default)
        #[arg(long)]
        model: Option<String>,
        /// System prompt (default: daemon's default)
        #[arg(long)]
        system_prompt: Option<String>,
    },
    /// Kill a session
    Kill {
        /// Session ID
        session_id: String,
    },
    /// Restart the daemon (checkpoint sessions, restart process)
    Restart,
    /// Tail daemon logs
    Logs {
        /// Follow log output (like tail -f)
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show (default: 50)
        #[arg(short, long, default_value_t = 50)]
        lines: usize,
    },
}
