//! Shared mode utilities

use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;

use tokio::sync::broadcast;
use tracing::info;
use tracing::warn;

use crate::agent::events::AgentEvent;
use crate::plugin::PluginManager;
use crate::plugin::PluginState;
use crate::tools::Tool;
use crate::tools::ToolDefinition;
use crate::tools::plugin_tool::PluginTool;
use crate::tools::validator_tool::ValidatorTool;

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
}

/// Build the default set of tools, wiring up channels from a [`ToolEnv`].
///
/// Per-tool streaming is handled uniformly via `ToolContext` — the event
/// channel is passed to every tool at execution time by the turn loop,
/// so no per-tool wiring is needed here.
pub fn build_tools_with_env(env: &ToolEnv) -> Vec<Arc<dyn Tool>> {
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

    let mut todo_tool = crate::tools::todo::TodoTool::new();
    if let Some(tx) = todo_tx {
        todo_tool = todo_tool.with_tx(tx);
    }

    let mut procmon_tool = crate::tools::procmon::ProcmonTool::new();
    if let Some(ref pm) = process_monitor {
        procmon_tool = procmon_tool.with_monitor(pm.clone());
    }

    vec![
        Arc::new(crate::tools::read::ReadTool::new()),
        Arc::new(crate::tools::write::WriteTool::new()),
        Arc::new(crate::tools::edit::EditTool::new()),
        Arc::new(bash_tool),
        Arc::new(crate::tools::grep::GrepTool::new()),
        Arc::new(crate::tools::find::FindTool::new()),
        Arc::new(crate::tools::ls::LsTool::new()),
        Arc::new(subagent_tool),
        Arc::new(delegate_tool),
        Arc::new(todo_tool),
        Arc::new(crate::tools::nix::NixTool::new()),
        Arc::new(crate::tools::web::WebTool::new()),
        Arc::new(crate::tools::commit::CommitTool::new()),
        Arc::new(crate::tools::review::ReviewTool::new()),
        Arc::new(crate::tools::ask::AskTool::new()),
        Arc::new(crate::tools::image_gen::ImageGenTool::new()),
        #[cfg(feature = "tui-validate")]
        Arc::new(crate::tools::devtools::validate_tui::ValidateTuiTool::new()),
        Arc::new(procmon_tool),
        // Matrix tools (always registered; they return helpful errors when not connected)
        Arc::new(crate::tools::matrix::MatrixSendTool::new()),
        Arc::new(crate::tools::matrix::MatrixReadTool::new()),
        Arc::new(crate::tools::matrix::MatrixRoomsTool::new()),
        Arc::new(crate::tools::matrix::MatrixPeersTool::new()),
        Arc::new(crate::tools::matrix::MatrixJoinTool::new()),
        Arc::new(crate::tools::matrix::MatrixRpcTool::new()),
        Arc::new(crate::tools::signal_loop::SignalLoopTool::new()),
    ]
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

    // Load all discovered plugins' WASM modules (skip disabled ones)
    let names: Vec<String> = manager.list().iter().map(|p| p.name.clone()).collect();
    for name in &names {
        if disabled.contains(name) {
            manager.disable(name).ok();
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
    let mgr = match manager.lock() {
        Ok(m) => m,
        Err(e) => {
            warn!("Failed to lock plugin manager: {}", e);
            return Vec::new();
        }
    };

    let mut tools: Vec<Arc<dyn Tool>> = Vec::new();

    // Derive built-in tool names from the actual tool list — skip plugin tools that collide
    let builtin_names: std::collections::HashSet<String> =
        builtin_tools.iter().map(|t| t.definition().name.clone()).collect();

    for plugin_info in mgr.list() {
        if plugin_info.state != PluginState::Active {
            continue;
        }

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
            let mut vtool = ValidatorTool::new(
                definition,
                plugin_info.name.clone(),
                tool_def.handler.clone(),
                Arc::clone(manager),
            );
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

/// Build the full tool set (built-in + plugin) from a [`ToolEnv`].
pub fn build_all_tools_with_env(
    env: &ToolEnv,
    plugin_manager: Option<&Arc<Mutex<PluginManager>>>,
) -> Vec<Arc<dyn Tool>> {
    let mut tools = build_tools_with_env(env);
    if let Some(manager) = plugin_manager {
        tools.extend(build_plugin_tools(&tools, manager, env.panel_tx.as_ref()));
    }
    tools
}

/// Fire `plugin_init` event to all active plugins that subscribe to it.
/// Returns the collected UI actions so the caller can apply them to the TUI.
pub fn fire_plugin_init(plugin_manager: &Arc<Mutex<PluginManager>>) -> Vec<crate::plugin::ui::PluginUIAction> {
    use crate::plugin::PluginState;
    use crate::plugin::bridge::PluginEvent;
    use crate::plugin::bridge::parse_ui_actions;

    let mgr = match plugin_manager.lock() {
        Ok(m) => m,
        Err(poisoned) => {
            tracing::warn!("Plugin manager mutex was poisoned during init, recovering");
            poisoned.into_inner()
        }
    };

    let mut actions = Vec::new();

    for plugin_info in mgr.list() {
        if plugin_info.state != PluginState::Active {
            continue;
        }

        // Check if this plugin has on_event and subscribes to plugin_init
        let subscribed =
            plugin_info.manifest.events.iter().any(|e| PluginEvent::parse(e) == Some(PluginEvent::PluginInit));
        if !subscribed {
            continue;
        }
        if !mgr.has_function(&plugin_info.name, "on_event") {
            continue;
        }

        let payload = serde_json::json!({"event": "plugin_init", "data": {}});
        let input = serde_json::to_string(&payload).unwrap_or_default();
        match mgr.call_plugin(&plugin_info.name, "on_event", &input) {
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
