//! Shared mode utilities

use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use crossterm::event::DisableBracketedPaste;
use crossterm::event::DisableFocusChange;
use crossterm::event::DisableMouseCapture;
use crossterm::event::EnableBracketedPaste;
use crossterm::event::EnableFocusChange;
use crossterm::event::EnableMouseCapture;
use crossterm::execute;
use crossterm::terminal::EnterAlternateScreen;
use crossterm::terminal::LeaveAlternateScreen;
use crossterm::terminal::{self};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::broadcast;
use tracing::info;
use tracing::warn;

use clankers_agent::events::AgentEvent;
use crate::error::Result;
use clankers_plugin::PluginManager;
use clankers_plugin::PluginState;
use crate::tools::Tool;

// ── Thinking setup ──────────────────────────────────────────────────────────

pub(crate) fn core_thinking_level(level: clanker_message::ThinkingLevel) -> clankers_core::CoreThinkingLevel {
    match level {
        clanker_message::ThinkingLevel::Off => clankers_core::CoreThinkingLevel::Off,
        clanker_message::ThinkingLevel::Low => clankers_core::CoreThinkingLevel::Low,
        clanker_message::ThinkingLevel::Medium => clankers_core::CoreThinkingLevel::Medium,
        clanker_message::ThinkingLevel::High => clankers_core::CoreThinkingLevel::High,
        clanker_message::ThinkingLevel::Max => clankers_core::CoreThinkingLevel::Max,
    }
}

pub(crate) fn apply_thinking_settings(app: &mut clankers_tui::app::App, settings: &clankers_config::settings::Settings) {
    let level = settings.parsed_thinking_level();
    app.thinking_enabled = level.is_enabled();
    app.thinking_level = level;
}

// ── Terminal setup ──────────────────────────────────────────────────────────

/// Set up the crossterm terminal (raw mode, alternate screen, mouse capture).
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    terminal::enable_raw_mode().map_err(|e| crate::error::Error::Tui {
        message: format!("Failed to enable raw mode: {e}"),
    })?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture, EnableBracketedPaste, EnableFocusChange).map_err(
        |e| crate::error::Error::Tui {
            message: format!("Failed to enter alternate screen: {e}"),
        },
    )?;
    let backend = CrosstermBackend::new(stdout);
    Terminal::new(backend).map_err(|e| crate::error::Error::Tui {
        message: format!("Failed to create terminal: {e}"),
    })
}

/// Tear down the crossterm terminal (restore normal mode).
pub fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) {
    terminal::disable_raw_mode().ok();
    execute!(
        terminal.backend_mut(),
        DisableBracketedPaste,
        DisableMouseCapture,
        DisableFocusChange,
        LeaveAlternateScreen,
    )
    .ok();
    terminal.show_cursor().ok();
}

// ── Tool tiers ──────────────────────────────────────────────────────────────

/// Tool tier classification. Tools are grouped by when they're needed.
/// Only active tiers are sent to the API, reducing schema token cost.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolTier {
    /// Core file/shell tools — always active (read, write, edit, patch, execute_code, process,
    /// bash, grep, find, ls)
    Core,
    /// Orchestration tools — subagent, delegate, signal_loop, procmon
    Orchestration,
    /// Specialty tools — nix, web, commit, review, ask, image_gen, todo, switch_model
    Specialty,
    /// Matrix tools — matrix_send, matrix_read, etc. (daemon only)
    Matrix,
}

/// A tiered collection of tools. Only tools in active tiers are sent to the API.
pub struct ToolSet {
    /// All tools with their tier assignment.
    all: Vec<(ToolTier, Arc<dyn Tool>)>,
    /// Currently active tiers.
    active: HashSet<ToolTier>,
}

impl ToolSet {
    /// Create a new ToolSet from a tiered tool list with specified active tiers.
    pub fn new(tiered_tools: Vec<(ToolTier, Arc<dyn Tool>)>, tiers: impl IntoIterator<Item = ToolTier>) -> Self {
        Self {
            all: tiered_tools,
            active: tiers.into_iter().collect(),
        }
    }

    /// Tools to send to the API on the current turn.
    pub fn active_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.all
            .iter()
            .filter(|(tier, _)| self.active.contains(tier))
            .map(|(_, tool)| tool.clone())
            .collect()
    }

    /// All tools regardless of tier (for /tools list, collision detection).
    pub fn all_tools(&self) -> Vec<Arc<dyn Tool>> {
        self.all.iter().map(|(_, tool)| tool.clone()).collect()
    }

    /// Get all tools with their tier info (for display in /tools).
    pub fn all_tools_with_tiers(&self) -> &[(ToolTier, Arc<dyn Tool>)] {
        &self.all
    }

    /// Activate a tier.
    pub fn activate(&mut self, tier: ToolTier) {
        self.active.insert(tier);
    }

    /// Deactivate a tier.
    pub fn deactivate(&mut self, tier: ToolTier) {
        self.active.remove(&tier);
    }

    /// Check if a tier is currently active.
    pub fn is_active(&self, tier: ToolTier) -> bool {
        self.active.contains(&tier)
    }

    /// Get the set of active tiers.
    pub fn active_tiers(&self) -> &HashSet<ToolTier> {
        &self.active
    }
}

impl std::fmt::Display for ToolTier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ToolTier::Core => write!(f, "core"),
            ToolTier::Orchestration => write!(f, "orchestration"),
            ToolTier::Specialty => write!(f, "specialty"),
            ToolTier::Matrix => write!(f, "matrix"),
        }
    }
}

impl std::str::FromStr for ToolTier {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "core" => Ok(ToolTier::Core),
            "orchestration" | "orch" => Ok(ToolTier::Orchestration),
            "specialty" | "spec" => Ok(ToolTier::Specialty),
            "matrix" => Ok(ToolTier::Matrix),
            _ => Err(format!("Unknown tool tier: {}", s)),
        }
    }
}

impl ToolTier {
    /// Parse a tier name from a string.
    pub fn parse_tier(s: &str) -> Option<Self> {
        s.parse().ok()
    }
}

/// Optional channels and handles that tools may use for live updates.
///
/// Passed as a single struct instead of 5+ positional `Option` parameters.
/// Use `Default::default()` for headless / test contexts.
#[derive(Default, Clone)]
pub struct ToolEnv {
    /// Settings needed by tool constructors that mirror runtime config.
    pub settings: Option<clankers_config::settings::Settings>,
    /// Event bus for streaming partial results to the TUI.
    pub event_tx: Option<broadcast::Sender<AgentEvent>>,
    /// Channel for subagent panel events (delegate/subagent status).
    pub panel_tx: Option<tokio::sync::mpsc::UnboundedSender<clankers_tui::components::subagent_event::SubagentEvent>>,
    /// Channel for TODO list updates.
    pub todo_tx: Option<crate::tools::todo::TodoTx>,
    /// Channel for bash tool confirmation prompts.
    pub bash_confirm_tx: Option<crate::tools::bash::ConfirmTx>,
    /// Shared process monitor for tracking child processes.
    pub process_monitor: Option<clankers_procmon::ProcessMonitorHandle>,
    /// Actor context for in-process subagent/delegate spawning (daemon mode).
    pub actor_ctx: Option<crate::tools::subagent::ActorContext>,
    /// Shared schedule engine for the schedule tool.
    ///
    /// In daemon mode this is created once and shared across sessions so
    /// schedules survive session restarts. In standalone mode a per-process
    /// engine is created.
    pub schedule_engine: Option<Arc<clanker_scheduler::ScheduleEngine>>,
    /// Shared MCP runtime registry for configured MCP servers.
    pub mcp_registry: Option<Arc<dyn crate::tools::mcp::McpRuntimeRegistry>>,
}

/// Build all tools with tier assignments, wiring up channels from a [`ToolEnv`].
///
/// Concrete tool construction is owned by `modes::tool_catalog`; this public
/// wrapper preserves the existing call surface while keeping family ownership
/// inspectable.
pub fn build_tiered_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    crate::modes::tool_catalog::build_builtin_tiered_tools(env)
}

/// Initialize the plugin manager, discover and load all plugins from the
/// given directories. Returns the manager wrapped in Arc<Mutex<>> for sharing.
///
/// Scans in order (later dirs override earlier):
/// 1. Global plugins dir (`~/.clankers/agent/plugins/`)
/// 2. Project config plugins (`.clankers/plugins/`)
/// 3. Any extra directories (e.g. project root `plugins/`)
pub fn init_plugin_manager(
    global_plugins_dir: &Path,
    project_plugins_dir: Option<&Path>,
    extra_dirs: &[&Path],
) -> Arc<Mutex<PluginManager>> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    init_plugin_manager_for_mode(
        global_plugins_dir,
        project_plugins_dir,
        extra_dirs,
        clankers_plugin::PluginRuntimeMode::Standalone,
        &cwd,
    )
}

pub fn init_plugin_manager_for_mode(
    global_plugins_dir: &Path,
    project_plugins_dir: Option<&Path>,
    extra_dirs: &[&Path],
    runtime_mode: clankers_plugin::PluginRuntimeMode,
    cwd: &Path,
) -> Arc<Mutex<PluginManager>> {
    let mut manager =
        PluginManager::new(global_plugins_dir.to_path_buf(), project_plugins_dir.map(|p| p.to_path_buf()));
    for dir in extra_dirs {
        manager.add_plugin_dir(dir.to_path_buf());
    }
    manager.discover();

    // Restore disabled plugins from persisted state
    let disabled = crate::slash_commands::handlers::tools::load_disabled_plugins();
    if !disabled.is_empty() {
        info!("Restoring {} disabled plugin(s): {}", disabled.len(), disabled.join(", "));
    }

    // Load all discovered Extism plugins' WASM modules (skip disabled or invalid entries)
    let names: Vec<String> = manager.list().iter().map(|p| p.name.clone()).collect();
    for name in &names {
        if disabled.contains(name) {
            manager.disable(name).ok();
            continue;
        }

        let Some((kind, has_manifest_error)) = manager
            .get(name)
            .map(|info| (info.manifest.kind.clone(), matches!(info.state, PluginState::Error(_))))
        else {
            continue;
        };

        if has_manifest_error {
            warn!("Skipping plugin '{}' due to manifest validation error", name);
            continue;
        }

        if !kind.uses_wasm_runtime() {
            info!("Discovered {} plugin '{}'; skipping WASM load", kind, name);
            continue;
        }

        match manager.load_wasm(name) {
            Ok(()) => info!("Loaded plugin: {}", name),
            Err(e) => warn!("Failed to load plugin '{}': {}", name, e),
        }
    }

    let builtin_tool_names = build_tiered_tools(&ToolEnv::default())
        .into_iter()
        .map(|(_, tool)| tool.definition().name.clone())
        .collect::<std::collections::HashSet<_>>();
    manager.set_stdio_reserved_tool_names(builtin_tool_names);

    let manager = Arc::new(Mutex::new(manager));
    clankers_plugin::configure_stdio_runtime(&manager, cwd.to_path_buf(), runtime_mode);
    clankers_plugin::start_stdio_plugins(&manager);
    manager
}

/// Build tools provided by loaded plugins. Each tool declared in a plugin's
/// manifest becomes a `PluginTool` that the agent can invoke. Validator plugins
/// (those with "exec" permission and validation tools) get the `ValidatorTool`
/// adapter that can spawn subprocess validators.
pub fn build_plugin_tools(
    builtin_tools: &[Arc<dyn Tool>],
    manager: &Arc<Mutex<PluginManager>>,
    panel_tx: Option<&tokio::sync::mpsc::UnboundedSender<clankers_tui::components::subagent_event::SubagentEvent>>,
) -> Vec<Arc<dyn Tool>> {
    crate::modes::tool_catalog::build_plugin_tools(builtin_tools, manager, panel_tx)
}

/// Build the full tiered tool set (built-in, plugin, MCP, and runtime extension tools).
pub fn build_all_tiered_tools(
    env: &ToolEnv,
    plugin_manager: Option<&Arc<Mutex<PluginManager>>>,
) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    crate::modes::tool_catalog::build_all_tiered_tools(env, plugin_manager)
}

/// Publish the existing Clankers tool registration as a host-facing runtime catalog.
///
/// This keeps embedders on the same source of truth as the CLI/TUI/daemon path:
/// built-ins come from [`build_tiered_tools`], plugin tools from [`build_plugin_tools`],
/// MCP and optional runtime tools come from the same tiered catalog owner.
pub fn runtime_tool_catalog_from_tiered_tools(
    tiered_tools: &[(ToolTier, Arc<dyn Tool>)],
    disabled_tools: &std::collections::HashSet<String>,
) -> Result<clankers_runtime::ToolCatalog, clankers_runtime::RuntimeError> {
    let mut builder = clankers_runtime::ToolCatalog::builder().disabled_tools(disabled_tools.iter().cloned());
    for (tier, tool) in tiered_tools {
        let definition = tool.definition();
        builder = builder.custom_tool(clankers_runtime::ToolDescriptor::new(
            definition.name.clone(),
            definition.description.clone(),
            side_effect_for_runtime_tool(*tier, definition.name.as_str()),
        ))?;
    }
    builder.build()
}

fn side_effect_for_runtime_tool(tier: ToolTier, name: &str) -> clankers_runtime::SideEffectLevel {
    match name {
        "read" | "grep" | "find" | "ls" | "session_search" | "skill_view" | "steel_eval" => {
            clankers_runtime::SideEffectLevel::ReadOnly
        }
        "write" | "edit" | "patch" => clankers_runtime::SideEffectLevel::WorkspaceMutation,
        "web" | "browser" | "image_gen" | "tts" | "external_memory" => clankers_runtime::SideEffectLevel::ExternalIo,
        "bash" | "execute_code" | "process" | "nix" | "nix_eval" | "commit" | "schedule" | "checkpoint"
        | "tool_gateway" | "voice_mode" | "mcp" => clankers_runtime::SideEffectLevel::Dangerous,
        _ => match tier {
            ToolTier::Core | ToolTier::Orchestration | ToolTier::Matrix => clankers_runtime::SideEffectLevel::Dangerous,
            ToolTier::Specialty => clankers_runtime::SideEffectLevel::ExternalIo,
        },
    }
}

/// Resolve active tiers from CLI `--tools` flag value.
///
/// Returns `None` when the caller should use mode defaults.
/// Returns `Some(tiers)` for explicit tier selection.
pub fn resolve_tool_tiers(tools_flag: Option<&str>) -> Option<Vec<ToolTier>> {
    match tools_flag {
        Some("all") => Some(vec![
            ToolTier::Core,
            ToolTier::Orchestration,
            ToolTier::Specialty,
            ToolTier::Matrix,
        ]),
        Some("core") => Some(vec![ToolTier::Core]),
        Some("none" | "") => None, // handled separately (empty tool vec)
        Some(custom) => {
            // Parse comma-separated tier names
            let tiers: Vec<ToolTier> = custom.split(',').filter_map(|s| ToolTier::parse_tier(s.trim())).collect();
            if tiers.is_empty() { None } else { Some(tiers) }
        }
        None => None,
    }
}

/// Fire `plugin_init` event to all active plugins that subscribe to it.
/// Returns the collected UI actions so the caller can apply them to the TUI.
pub fn fire_plugin_init(plugin_manager: &Arc<Mutex<PluginManager>>) -> Vec<clankers_plugin::ui::PluginUiAction> {
    use clankers_plugin::bridge::parse_ui_actions;

    let host = clankers_plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
    let mut actions = Vec::new();

    for plugin_info in host.event_subscribers("plugin_init") {
        if !host.has_function(&plugin_info.name, "on_event") {
            continue;
        }

        let payload = serde_json::json!({"event": "plugin_init", "data": {}});
        let input = serde_json::to_string(&payload).unwrap_or_default();
        match host.call_plugin(&plugin_info.name, "on_event", &input) {
            Ok(output) => {
                if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&output) {
                    let plugin_actions = parse_ui_actions(&plugin_info.name, &parsed);
                    let plugin_actions =
                        clankers_plugin::sandbox::filter_ui_actions(&plugin_info.manifest.permissions, plugin_actions);
                    actions.extend(plugin_actions);
                }
            }
            Err(e) => {
                tracing::debug!("Plugin '{}' init error: {}", plugin_info.name, e);
            }
        }
    }

    actions
}

// Provider discovery re-exports (canonical home: `clankers_provider::discovery`)
pub use clankers_provider::discovery::build_router;
pub use clankers_provider::discovery::build_router_with_rpc;

// ─── Headless helpers ───────────────────────────────────────────────────────

/// Build a context prefix from `--attach` file paths.
///
/// Returns a string to prepend to the user prompt that contains the contents
/// of all attached files, formatted as labeled code blocks.
pub fn build_attach_context(attach_paths: &[String]) -> String {
    if attach_paths.is_empty() {
        return String::new();
    }

    let mut parts = Vec::new();
    for path_str in attach_paths {
        let path = PathBuf::from(path_str);
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                parts.push(format!(
                    "<attached_file path=\"{}\">\n```{}\n{}\n```\n</attached_file>",
                    path.display(),
                    ext,
                    content.trim_end(),
                ));
            }
            Err(e) => {
                // Binary or unreadable — include the error so the LLM knows
                parts.push(format!(
                    "<attached_file path=\"{}\">\n(could not read: {})\n</attached_file>",
                    path.display(),
                    e,
                ));
            }
        }
    }

    format!("The following files have been attached for context:\n\n{}\n\n", parts.join("\n\n"),)
}

/// Headless-mode configuration aggregated from CLI flags.
/// Centralises all the bits that print / json / markdown modes need.
#[derive(Debug, Clone)]
pub struct HeadlessConfig {
    /// Resolved prompt text (including attached file context)
    pub prompt: String,
    /// Output file (from `--output`)
    pub output_file: Option<String>,
    /// Show token usage stats (from `--stats`)
    pub show_stats: bool,
    /// Maximum agent loop iterations (from `--max-iterations`)
    pub max_iterations: usize,
    /// Show tool calls in output (from `--verbose`)
    pub show_tools: bool,
    /// Session persistence directory (None = no persistence)
    pub sessions_dir: Option<PathBuf>,
    /// Working directory (for session metadata)
    pub cwd: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // Create a minimal mock tool for testing ToolSet
    struct MockTool {
        name: String,
    }

    impl MockTool {
        fn new(name: &str) -> Self {
            Self { name: name.to_string() }
        }
    }

    #[async_trait::async_trait]
    impl crate::tools::Tool for MockTool {
        fn definition(&self) -> &crate::tools::ToolDefinition {
            // Leak is fine in tests
            Box::leak(Box::new(crate::tools::ToolDefinition {
                name: self.name.clone(),
                description: format!("Mock tool: {}", self.name),
                input_schema: serde_json::json!({"type": "object"}),
            }))
        }

        async fn execute(
            &self,
            _ctx: &clankers_agent::tool::ToolContext,
            _params: serde_json::Value,
        ) -> clankers_agent::tool::ToolResult {
            clankers_agent::tool::ToolResult::text("ok")
        }
    }

    fn make_tiered_tools() -> Vec<(ToolTier, Arc<dyn crate::tools::Tool>)> {
        vec![
            (ToolTier::Core, Arc::new(MockTool::new("read"))),
            (ToolTier::Core, Arc::new(MockTool::new("write"))),
            (ToolTier::Core, Arc::new(MockTool::new("bash"))),
            (ToolTier::Orchestration, Arc::new(MockTool::new("subagent"))),
            (ToolTier::Orchestration, Arc::new(MockTool::new("delegate_task"))),
            (ToolTier::Specialty, Arc::new(MockTool::new("nix"))),
            (ToolTier::Specialty, Arc::new(MockTool::new("web"))),
            (ToolTier::Specialty, Arc::new(MockTool::new("commit"))),
            (ToolTier::Matrix, Arc::new(MockTool::new("matrix_send"))),
            (ToolTier::Matrix, Arc::new(MockTool::new("matrix_read"))),
        ]
    }

    fn tool_names(tools: &[Arc<dyn crate::tools::Tool>]) -> Vec<String> {
        tools.iter().map(|t| t.definition().name.clone()).collect()
    }

    struct FakeMcpRegistry;

    #[async_trait::async_trait]
    impl crate::tools::mcp::McpRuntime for FakeMcpRegistry {
        async fn call_tool(
            &self,
            _server: &str,
            _tool: &str,
            _args: serde_json::Value,
        ) -> Result<serde_json::Value, String> {
            Ok(serde_json::json!({"content": [{"type": "text", "text": "ok"}]}))
        }
    }

    impl crate::tools::mcp::McpRuntimeRegistry for FakeMcpRegistry {
        fn registered_tools(&self, server: &str) -> Vec<crate::tools::mcp::McpRegisteredTool> {
            match server {
                "filesystem" => vec![crate::tools::mcp::McpRegisteredTool::new(
                    "read_file",
                    "Read a file",
                    serde_json::json!({"type": "object"}),
                )],
                "colliding" => vec![crate::tools::mcp::McpRegisteredTool::new(
                    "read_file",
                    "Also reads a file",
                    serde_json::json!({"type": "object"}),
                )],
                _ => Vec::new(),
            }
        }
    }

    #[test]
    fn build_tiered_tools_publishes_checkpoint_specialty_tool() {
        let tiered = build_tiered_tools(&ToolEnv::default());

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "checkpoint")
        );
    }

    #[test]
    fn build_tiered_tools_publishes_tool_gateway_specialty_tool() {
        let tiered = build_tiered_tools(&ToolEnv::default());

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "tool_gateway")
        );
    }

    #[test]
    fn runtime_catalog_matches_existing_default_tool_registration() {
        let tiered = build_all_tiered_tools(&ToolEnv::default(), None);
        let source_names = tiered
            .iter()
            .map(|(_, tool)| tool.definition().name.clone())
            .collect::<std::collections::BTreeSet<_>>();

        let catalog = runtime_tool_catalog_from_tiered_tools(&tiered, &std::collections::HashSet::new())
            .expect("default tool registration should publish runtime catalog");
        let catalog_names = catalog.tools().map(|tool| tool.name.clone()).collect::<std::collections::BTreeSet<_>>();

        assert_eq!(catalog_names, source_names);
        assert!(catalog.contains_tool("tool_gateway"));
        assert!(catalog.contains_tool("bash"));
        assert!(catalog.tools().find(|tool| tool.name == "bash").expect("bash descriptor").requires_confirmation);
    }

    #[test]
    fn runtime_catalog_tracks_mcp_and_disabled_tool_publication() {
        let mut settings = clankers_config::settings::Settings::default();
        settings.mcp.servers.insert("filesystem".to_string(), clankers_config::McpServerConfig {
            enabled: true,
            transport: clankers_config::McpTransport::Stdio,
            command: Some("fake-mcp".to_string()),
            args: Vec::new(),
            url: None,
            env_allowlist: Vec::new(),
            header_env: std::collections::BTreeMap::new(),
            include_tools: Vec::new(),
            exclude_tools: Vec::new(),
            tool_prefix: None,
            timeout_ms: None,
        });
        let env = ToolEnv {
            settings: Some(settings),
            mcp_registry: Some(Arc::new(FakeMcpRegistry)),
            ..Default::default()
        };
        let tiered = build_all_tiered_tools(&env, None);
        assert!(tiered.iter().any(|(_, tool)| tool.definition().name == "mcp_filesystem_read_file"));

        let disabled = std::collections::HashSet::from(["bash".to_string()]);
        let catalog = runtime_tool_catalog_from_tiered_tools(&tiered, &disabled)
            .expect("MCP publication should flow through runtime catalog");

        assert!(catalog.contains_tool("mcp_filesystem_read_file"));
        assert!(!catalog.contains_tool("bash"));
    }

    #[test]
    fn build_tiered_tools_publishes_voice_mode_specialty_tool() {
        let tiered = build_tiered_tools(&ToolEnv::default());

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "voice_mode")
        );
    }

    #[test]
    fn build_tiered_tools_publishes_soul_personality_specialty_tool() {
        let tiered = build_tiered_tools(&ToolEnv::default());

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "soul_personality")
        );
    }

    #[test]
    fn build_tiered_tools_publishes_browser_only_when_enabled() {
        let default_env = ToolEnv::default();
        let default_names: Vec<String> = build_tiered_tools(&default_env)
            .into_iter()
            .map(|(_, tool)| tool.definition().name.clone())
            .collect();
        assert!(!default_names.iter().any(|name| name == "browser"));

        let mut settings = clankers_config::settings::Settings::default();
        settings.browser_automation.enabled = true;
        settings.browser_automation.cdp_url = Some("http://127.0.0.1:9222".to_string());
        let env = ToolEnv {
            settings: Some(settings),
            ..Default::default()
        };
        let tiered = build_tiered_tools(&env);

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "browser")
        );
    }

    #[test]
    fn build_tiered_tools_publishes_steel_eval_by_default_with_explicit_opt_out() {
        let mut settings = clankers_config::settings::Settings::default();
        let env = ToolEnv {
            settings: Some(settings.clone()),
            ..Default::default()
        };
        let default_names: Vec<String> =
            build_tiered_tools(&env).into_iter().map(|(_, tool)| tool.definition().name.clone()).collect();
        assert!(default_names.iter().any(|name| name == "steel_eval"));

        settings.steel_eval.enabled = false;
        let disabled_env = ToolEnv {
            settings: Some(settings),
            ..Default::default()
        };
        let disabled_names: Vec<String> = build_tiered_tools(&disabled_env)
            .into_iter()
            .map(|(_, tool)| tool.definition().name.clone())
            .collect();
        assert!(!disabled_names.iter().any(|name| name == "steel_eval"));
    }

    #[test]
    fn steel_eval_uses_standard_disabled_tool_filter() {
        let mut settings = clankers_config::settings::Settings::default();
        settings.steel_eval.enabled = true;
        let env = ToolEnv {
            settings: Some(settings),
            ..Default::default()
        };
        let tiered = build_tiered_tools(&env);
        let disabled = HashSet::from(["steel_eval".to_string()]);
        let allowed = crate::tool_gateway::allowed_tools_for_policy(
            &tiered,
            &crate::tool_gateway::standalone_toolsets(),
            &disabled,
        );
        let names = tool_names(&allowed);

        assert!(!names.contains(&"steel_eval".to_string()));
    }

    #[test]
    fn build_tiered_tools_publishes_external_memory_only_when_enabled() {
        let default_env = ToolEnv::default();
        let default_names: Vec<String> = build_tiered_tools(&default_env)
            .into_iter()
            .map(|(_, tool)| tool.definition().name.clone())
            .collect();
        assert!(!default_names.iter().any(|name| name == "external_memory"));

        let mut settings = clankers_config::settings::Settings::default();
        settings.external_memory.enabled = true;
        let env = ToolEnv {
            settings: Some(settings),
            ..Default::default()
        };
        let tiered = build_tiered_tools(&env);

        assert!(
            tiered
                .iter()
                .any(|(tier, tool)| *tier == ToolTier::Specialty && tool.definition().name == "external_memory")
        );
    }

    #[test]
    fn build_tiered_tools_skips_invalid_external_memory_config() {
        let mut settings = clankers_config::settings::Settings::default();
        settings.external_memory.enabled = true;
        settings.external_memory.max_results = 0;
        let env = ToolEnv {
            settings: Some(settings),
            ..Default::default()
        };
        let names: Vec<String> =
            build_tiered_tools(&env).into_iter().map(|(_, tool)| tool.definition().name.clone()).collect();

        assert!(!names.iter().any(|name| name == "external_memory"));
    }

    #[test]
    fn build_all_tiered_tools_adds_mcp_specialty_tools_from_settings() {
        let settings = clankers_config::settings::Settings {
            mcp: serde_json::from_value(serde_json::json!({
                "servers": {
                    "filesystem": {"transport": "stdio", "command": "fake-mcp", "toolPrefix": "fs"}
                }
            }))
            .unwrap(),
            ..Default::default()
        };
        let env = ToolEnv {
            settings: Some(settings),
            mcp_registry: Some(Arc::new(FakeMcpRegistry)),
            ..Default::default()
        };

        let tiered = build_all_tiered_tools(&env, None);
        let tool_set = ToolSet::new(tiered, [ToolTier::Specialty]);
        let names = tool_names(&tool_set.active_tools());

        assert!(names.contains(&"fs_read_file".to_string()));
    }

    #[test]
    fn build_all_tiered_tools_skips_mcp_tools_that_collide_with_builtins() {
        let settings = clankers_config::settings::Settings {
            mcp: serde_json::from_value(serde_json::json!({
                "servers": {
                    "filesystem": {"transport": "stdio", "command": "fake-mcp", "toolPrefix": "fs"},
                    "colliding": {"transport": "stdio", "command": "fake-mcp", "toolPrefix": "fs"}
                }
            }))
            .unwrap(),
            ..Default::default()
        };
        let env = ToolEnv {
            settings: Some(settings),
            mcp_registry: Some(Arc::new(FakeMcpRegistry)),
            ..Default::default()
        };

        let tiered = build_all_tiered_tools(&env, None);
        let names: Vec<String> = tiered.iter().map(|(_, tool)| tool.definition().name.clone()).collect();

        assert_eq!(names.iter().filter(|name| name.as_str() == "fs_read_file").count(), 1);
    }

    #[test]
    fn tool_set_core_only() {
        let ts = ToolSet::new(make_tiered_tools(), [ToolTier::Core]);
        let names = tool_names(&ts.active_tools());
        assert_eq!(names, vec!["read", "write", "bash"]);
    }

    #[test]
    fn tool_set_all_tiers() {
        let ts = ToolSet::new(make_tiered_tools(), [
            ToolTier::Core,
            ToolTier::Orchestration,
            ToolTier::Specialty,
            ToolTier::Matrix,
        ]);
        let active = ts.active_tools();
        assert_eq!(active.len(), 10);
    }

    #[test]
    fn tool_set_activate_deactivate() {
        let mut ts = ToolSet::new(make_tiered_tools(), [ToolTier::Core]);
        assert_eq!(ts.active_tools().len(), 3);

        ts.activate(ToolTier::Orchestration);
        assert_eq!(ts.active_tools().len(), 5);

        ts.deactivate(ToolTier::Core);
        assert_eq!(ts.active_tools().len(), 2);
        let names = tool_names(&ts.active_tools());
        assert!(names.contains(&"subagent".to_string()));
    }

    #[test]
    fn tool_set_all_tools_ignores_tiers() {
        let ts = ToolSet::new(make_tiered_tools(), [ToolTier::Core]);
        // active_tools = 3 (core only), all_tools = 10
        assert_eq!(ts.active_tools().len(), 3);
        assert_eq!(ts.all_tools().len(), 10);
    }

    #[test]
    fn tool_set_collision_uses_all() {
        let ts = ToolSet::new(make_tiered_tools(), [ToolTier::Core]);
        // Collision detection should use all_tools, not active_tools
        let all_names: HashSet<String> = ts.all_tools().iter().map(|t| t.definition().name.clone()).collect();
        assert!(all_names.contains("matrix_send")); // Matrix tier not active, but in all_tools
    }

    #[test]
    fn tool_set_is_active() {
        let ts = ToolSet::new(make_tiered_tools(), [ToolTier::Core, ToolTier::Specialty]);
        assert!(ts.is_active(ToolTier::Core));
        assert!(ts.is_active(ToolTier::Specialty));
        assert!(!ts.is_active(ToolTier::Orchestration));
        assert!(!ts.is_active(ToolTier::Matrix));
    }

    #[test]
    fn tool_tier_display() {
        assert_eq!(format!("{}", ToolTier::Core), "core");
        assert_eq!(format!("{}", ToolTier::Orchestration), "orchestration");
        assert_eq!(format!("{}", ToolTier::Specialty), "specialty");
        assert_eq!(format!("{}", ToolTier::Matrix), "matrix");
    }

    #[test]
    fn tool_tier_from_str() {
        assert_eq!(ToolTier::parse_tier("core"), Some(ToolTier::Core));
        assert_eq!(ToolTier::parse_tier("Core"), Some(ToolTier::Core));
        assert_eq!(ToolTier::parse_tier("orchestration"), Some(ToolTier::Orchestration));
        assert_eq!(ToolTier::parse_tier("orch"), Some(ToolTier::Orchestration));
        assert_eq!(ToolTier::parse_tier("specialty"), Some(ToolTier::Specialty));
        assert_eq!(ToolTier::parse_tier("spec"), Some(ToolTier::Specialty));
        assert_eq!(ToolTier::parse_tier("matrix"), Some(ToolTier::Matrix));
        assert_eq!(ToolTier::parse_tier("unknown"), None);
    }

    #[test]
    fn resolve_tool_tiers_all() {
        let tiers = resolve_tool_tiers(Some("all")).unwrap();
        assert_eq!(tiers.len(), 4);
    }

    #[test]
    fn resolve_tool_tiers_core() {
        let tiers = resolve_tool_tiers(Some("core")).unwrap();
        assert_eq!(tiers, vec![ToolTier::Core]);
    }

    #[test]
    fn resolve_tool_tiers_none_returns_none() {
        assert!(resolve_tool_tiers(Some("none")).is_none());
        assert!(resolve_tool_tiers(None).is_none());
    }

    #[test]
    fn resolve_tool_tiers_comma_separated() {
        let tiers = resolve_tool_tiers(Some("core,orchestration")).unwrap();
        assert_eq!(tiers.len(), 2);
        assert!(tiers.contains(&ToolTier::Core));
        assert!(tiers.contains(&ToolTier::Orchestration));
    }

    #[test]
    fn resolve_tool_tiers_unknown_names_skipped() {
        // When all names are unknown, returns None (not an empty Some)
        assert!(resolve_tool_tiers(Some("read,write,bash")).is_none());
    }
}
