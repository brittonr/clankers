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

use crate::agent::events::AgentEvent;
use crate::error::Result;
use crate::plugin::PluginManager;
use crate::plugin::PluginState;
use crate::tools::Tool;
use crate::tools::ToolDefinition;
use crate::tools::plugin_tool::PluginTool;
use crate::tools::validator_tool::ValidatorTool;

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
    /// Core file/shell tools — always active (read, write, edit, bash, grep, find, ls)
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
    /// Event bus for streaming partial results to the TUI.
    pub event_tx: Option<broadcast::Sender<AgentEvent>>,
    /// Channel for subagent panel events (delegate/subagent status).
    pub panel_tx: Option<tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>>,
    /// Channel for TODO list updates.
    pub todo_tx: Option<crate::tools::todo::TodoTx>,
    /// Channel for bash tool confirmation prompts.
    pub bash_confirm_tx: Option<crate::tools::bash::ConfirmTx>,
    /// Shared process monitor for tracking child processes.
    pub process_monitor: Option<crate::procmon::ProcessMonitorHandle>,
    /// Actor context for in-process subagent/delegate spawning (daemon mode).
    pub actor_ctx: Option<crate::tools::subagent::ActorContext>,
    /// Shared schedule engine for the schedule tool.
    ///
    /// In daemon mode this is created once and shared across sessions so
    /// schedules survive session restarts. In standalone mode a per-process
    /// engine is created.
    pub schedule_engine: Option<Arc<clanker_scheduler::ScheduleEngine>>,
}

/// Build all tools with tier assignments, wiring up channels from a [`ToolEnv`].
///
/// Per-tool streaming is handled uniformly via `ToolContext` — the event
/// channel is passed to every tool at execution time by the turn loop,
/// so no per-tool wiring is needed here.
#[cfg_attr(dylint_lib = "tigerstyle", allow(function_length, reason = "sequential setup/dispatch logic — splitting would fragment readability"))]
pub fn build_tiered_tools(env: &ToolEnv) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let panel_tx = env.panel_tx.clone();
    let todo_tx = env.todo_tx.clone();
    let bash_confirm_tx = env.bash_confirm_tx.clone();
    let process_monitor = env.process_monitor.clone();
    let mut bash_tool = if let Some(tx) = bash_confirm_tx {
        crate::tools::bash::BashTool::with_confirm(tx)
    } else {
        crate::tools::bash::BashTool::new()
    };
    if let Some(ref pm) = process_monitor {
        bash_tool = bash_tool.with_process_monitor(pm.clone());
    }

    let mut subagent_tool = crate::tools::subagent::SubagentTool::new();
    if let Some(ref ptx) = panel_tx {
        subagent_tool = subagent_tool.with_panel_tx(ptx.clone());
    }
    if let Some(ref pm) = process_monitor {
        subagent_tool = subagent_tool.with_process_monitor(pm.clone());
    }
    if let Some(ref actx) = env.actor_ctx {
        subagent_tool = subagent_tool.with_actor_ctx(actx.clone());
    }

    let mut delegate_tool = crate::tools::delegate::DelegateTool::new();
    if let Some(ref ptx) = panel_tx {
        delegate_tool = delegate_tool.with_panel_tx(ptx.clone());
    }
    // Enable remote peer routing if paths exist
    {
        let paths = crate::config::ClankersPaths::get();
        let registry_path = crate::modes::rpc::peers::registry_path(paths);
        let identity_path = crate::modes::rpc::iroh::identity_path(paths);
        delegate_tool = delegate_tool.with_peer_routing(registry_path, identity_path);
    }
    if let Some(ref pm) = process_monitor {
        delegate_tool = delegate_tool.with_process_monitor(pm.clone());
    }
    if let Some(ref actx) = env.actor_ctx {
        delegate_tool = delegate_tool.with_actor_ctx(actx.clone());
    }

    let mut todo_tool = crate::tools::todo::TodoTool::new();
    if let Some(tx) = todo_tx {
        todo_tool = todo_tool.with_tx(tx);
    }

    let mut procmon_tool = crate::tools::procmon::ProcmonTool::new();
    if let Some(ref pm) = process_monitor {
        procmon_tool = procmon_tool.with_monitor(pm.clone());
    }

    let mut tools: Vec<(ToolTier, Arc<dyn Tool>)> = vec![
        // ── Core (always active) ────────────────────────────────────
        (ToolTier::Core, Arc::new(crate::tools::read::ReadTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::write::WriteTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::edit::EditTool::new())),
        (ToolTier::Core, Arc::new(bash_tool)),
        (ToolTier::Core, Arc::new(crate::tools::grep::GrepTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::find::FindTool::new())),
        (ToolTier::Core, Arc::new(crate::tools::ls::LsTool::new())),
        // ── Orchestration (on demand) ───────────────────────────────
        (ToolTier::Orchestration, Arc::new(subagent_tool)),
        (ToolTier::Orchestration, Arc::new(delegate_tool)),
        (ToolTier::Orchestration, Arc::new(crate::tools::signal_loop::SignalLoopTool::new())),
        (ToolTier::Orchestration, Arc::new(procmon_tool)),
        // ── Specialty (interactive default) ─────────────────────────
        (ToolTier::Specialty, Arc::new(todo_tool)),
        (ToolTier::Specialty, Arc::new(crate::tools::nix::NixTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::web::WebTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::commit::CommitTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::review::ReviewTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::ask::AskTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::image_gen::ImageGenTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::tts::TtsTool::new())),
        (ToolTier::Specialty, Arc::new(crate::tools::memory::MemoryTool::new(
            clankers_config::settings::MemoryLimits::default(),
        ))),
        (ToolTier::Specialty, Arc::new(crate::tools::skill_manage::SkillManageTool::new(
            crate::config::ClankersPaths::get().global_skills_dir.clone(),
        ))),
        (ToolTier::Specialty, Arc::new(crate::tools::session_search::SessionSearchTool::new(
            crate::config::ClankersPaths::get().global_sessions_dir.clone(),
            100,
        ))),
        (ToolTier::Specialty, Arc::new(crate::tools::compress::CompressTool::new(
            crate::tools::compress::compression_slot(),
            4,
            5,
        ))),
        // ── Matrix (daemon only) ────────────────────────────────────
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixSendTool::new())),
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixReadTool::new())),
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixRoomsTool::new())),
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixPeersTool::new())),
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixJoinTool::new())),
        (ToolTier::Matrix, Arc::new(crate::tools::matrix::MatrixRpcTool::new())),
    ];

    // Register schedule tool when a ScheduleEngine is available
    if let Some(ref engine) = env.schedule_engine {
        tools.push((ToolTier::Specialty, Arc::new(
            crate::tools::schedule::ScheduleTool::new(Arc::clone(engine)),
        )));
    }

    // Register nix_eval only when nix is on PATH
    if std::process::Command::new("nix")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok()
    {
        tools.push((ToolTier::Specialty, Arc::new(crate::tools::nix::eval_tool::NixEvalTool::new())));
    }

    #[cfg(feature = "tui-validate")]
    tools.push((ToolTier::Specialty, Arc::new(crate::tools::devtools::validate_tui::ValidateTuiTool::new())));

    tools
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

    Arc::new(Mutex::new(manager))
}

/// Build tools provided by loaded plugins. Each tool declared in a plugin's
/// manifest becomes a `PluginTool` that the agent can invoke. Validator plugins
/// (those with "exec" permission and validation tools) get the `ValidatorTool`
/// adapter that can spawn subprocess validators.
pub fn build_plugin_tools(
    builtin_tools: &[Arc<dyn Tool>],
    manager: &Arc<Mutex<PluginManager>>,
    panel_tx: Option<&tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>>,
) -> Vec<Arc<dyn Tool>> {
    let host = crate::plugin::PluginHostFacade::new(Arc::clone(manager));
    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    // Derive built-in tool names from the actual tool list — skip plugin tools that collide
    let builtin_names: std::collections::HashSet<String> =
        builtin_tools.iter().map(|t| t.definition().name.clone()).collect();

    for plugin_info in host.active_plugins() {
        if !plugin_info.manifest.tool_definitions.is_empty() {
            build_detailed_tools(&plugin_info, manager, &builtin_names, panel_tx, &mut tools);
        } else {
            build_bare_tools(&plugin_info, manager, &mut tools);
        }
    }

    if !tools.is_empty() {
        info!("Registered {} plugin tool(s)", tools.len());
    }

    tools
}

/// Build tools from detailed tool_definitions in a plugin manifest.
fn build_detailed_tools(
    plugin_info: &crate::plugin::PluginInfo,
    manager: &Arc<Mutex<PluginManager>>,
    builtin_names: &std::collections::HashSet<String>,
    panel_tx: Option<&tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>>,
    tools: &mut Vec<Arc<dyn Tool>>,
) {
    let is_validator = plugin_info.manifest.permissions.iter().any(|p| p == "exec" || p == "all");

    for tool_def in &plugin_info.manifest.tool_definitions {
        if builtin_names.contains(&tool_def.name) {
            continue;
        }

        let definition = ToolDefinition {
            name: tool_def.name.clone(),
            description: tool_def.description.clone(),
            input_schema: tool_def.input_schema.clone(),
        };

        if is_validator && tool_def.name.starts_with("validate") {
            let mut vtool =
                ValidatorTool::new(definition, plugin_info.name.clone(), tool_def.handler.clone(), Arc::clone(manager));
            if let Some(ptx) = panel_tx {
                vtool = vtool.with_panel_tx(ptx.clone());
            }
            tools.push(Arc::new(vtool));
        } else {
            tools.push(Arc::new(PluginTool::new(
                definition,
                plugin_info.name.clone(),
                tool_def.handler.clone(),
                Arc::clone(manager),
            )));
        }
    }
}

/// Build tools from bare tool names in a plugin manifest (fallback path).
fn build_bare_tools(
    plugin_info: &crate::plugin::PluginInfo,
    manager: &Arc<Mutex<PluginManager>>,
    tools: &mut Vec<Arc<dyn Tool>>,
) {
    for tool_name in &plugin_info.manifest.tools {
        let definition = ToolDefinition {
            name: tool_name.clone(),
            description: format!("Tool '{}' provided by plugin '{}'", tool_name, plugin_info.name),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "input": {
                        "type": "string",
                        "description": "Input to pass to the tool"
                    }
                }
            }),
        };
        tools.push(Arc::new(PluginTool::new(
            definition,
            plugin_info.name.clone(),
            "handle_tool_call".to_string(),
            Arc::clone(manager),
        )));
    }
}

/// Build the full tiered tool set (built-in with tiers + plugin tools as Specialty).
pub fn build_all_tiered_tools(
    env: &ToolEnv,
    plugin_manager: Option<&Arc<Mutex<PluginManager>>>,
) -> Vec<(ToolTier, Arc<dyn Tool>)> {
    let mut tiered = build_tiered_tools(env);
    if let Some(manager) = plugin_manager {
        let flat_tools: Vec<Arc<dyn Tool>> = tiered.iter().map(|(_, t)| t.clone()).collect();
        let plugin_tools = build_plugin_tools(&flat_tools, manager, env.panel_tx.as_ref());
        // Plugin tools are Specialty tier by default
        for tool in plugin_tools {
            tiered.push((ToolTier::Specialty, tool));
        }
    }
    tiered
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
pub fn fire_plugin_init(plugin_manager: &Arc<Mutex<PluginManager>>) -> Vec<crate::plugin::ui::PluginUiAction> {
    use crate::plugin::bridge::parse_ui_actions;

    let host = crate::plugin::PluginHostFacade::new(Arc::clone(plugin_manager));
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
                        crate::plugin::sandbox::filter_ui_actions(&plugin_info.manifest.permissions, plugin_actions);
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

// Provider discovery re-exports (canonical home: `crate::provider::discovery`)
pub use crate::provider::discovery::build_router;
pub use crate::provider::discovery::build_router_with_rpc;

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
            _ctx: &crate::agent::tool::ToolContext,
            _params: serde_json::Value,
        ) -> crate::agent::tool::ToolResult {
            crate::agent::tool::ToolResult::text("ok")
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
