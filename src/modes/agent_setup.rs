//! Agent construction — build the Agent with all tools wired up.
//!
//! Extracted from interactive.rs to isolate tool construction, process
//! monitor wiring, and agent builder configuration.

use std::sync::Arc;

use crate::agent::Agent;
use crate::agent::events::AgentEvent;
use crate::tui::app::App;

/// Build agent with all tools and configuration.
///
/// Creates the process monitor, wires it into the TUI panel, constructs
/// all tools (filtering disabled ones), and builds the final agent with
/// optional DB, routing, and cost tracking.
#[allow(clippy::type_complexity)]
pub(crate) fn build_agent_with_tools(
    provider: Arc<dyn crate::provider::Provider>,
    settings: &crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    app: &mut App,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    todo_tx: tokio::sync::mpsc::UnboundedSender<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    paths: &crate::config::ClankersPaths,
    db: &Option<crate::db::Db>,
) -> (Agent, tokio::sync::broadcast::Receiver<AgentEvent>, crate::tools::bash::ConfirmRx) {
    // Create a temporary agent with empty tools to get event_tx for tool construction
    let temp_agent =
        Agent::new(Arc::clone(&provider), Vec::new(), settings.clone(), model.clone(), system_prompt.clone());
    let event_tx = temp_agent.event_sender();
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Create and start the process monitor (bridges ProcessEvent → AgentEvent)
    let process_monitor = create_process_monitor(event_tx.clone());

    // Wire process monitor into the TUI panel
    *process_panel(app) = crate::tui::components::process_panel::ProcessPanel::new()
        .with_monitor(process_monitor.clone() as Arc<dyn clankers_tui_types::ProcessDataSource>);

    let tool_env = crate::modes::common::ToolEnv {
        event_tx: Some(event_tx),
        panel_tx: Some(panel_tx),
        todo_tx: Some(todo_tx),
        bash_confirm_tx: Some(bash_confirm_tx),
        process_monitor: Some(process_monitor),
        actor_ctx: None,
    };
    let tiered_tools = crate::modes::common::build_all_tiered_tools(&tool_env, plugin_manager);

    // Interactive mode: Core + Specialty + Orchestration (no Matrix)
    let tool_set = crate::modes::common::ToolSet::new(tiered_tools, [
        crate::modes::common::ToolTier::Core,
        crate::modes::common::ToolTier::Specialty,
        crate::modes::common::ToolTier::Orchestration,
    ]);

    // tool_info shows ALL tools (for toggle menu); active_tools only sends active tiers to agent
    let all_tools = tool_set.all_tools();
    populate_tool_info(app, &all_tools);

    // Fire plugin_init event so plugins can set up their initial UI
    if let Some(pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            crate::plugin::ui::apply_ui_action(&mut app.plugin_ui, action);
        }
    }

    // Filter out disabled tools from the active tier set
    let active_tools: Vec<Arc<dyn crate::tools::Tool>> = tool_set
        .active_tools()
        .into_iter()
        .filter(|t| !app.disabled_tools.contains(&t.definition().name))
        .collect();

    // Build the final agent with tools, db, routing, and cost tracking
    let mut agent_builder = crate::agent::builder::AgentBuilder::new(provider, settings.clone(), model, system_prompt)
        .with_tools(active_tools)
        .with_paths(paths.clone());

    // Apply default capability restrictions from settings
    if let Some(caps) = &settings.default_capabilities {
        let gate = std::sync::Arc::new(
            crate::capability_gate::UcanCapabilityGate::new(caps.clone()),
        );
        agent_builder = agent_builder.with_capability_gate(gate);
    }

    // Attach the global database so the agent can read memories and record usage
    if let Some(db) = db {
        agent_builder = agent_builder.with_db(db.clone());
    }

    // Build the agent (automatically wires routing and cost tracking from settings)
    let agent = agent_builder.build();

    // Extract cost tracker reference for the app UI
    if settings.cost_tracking.is_some() {
        app.cost_tracker = agent.cost_tracker().map(|ct| ct.clone() as Arc<dyn clankers_tui_types::CostProvider>);
    }

    let event_rx = agent.subscribe();

    (agent, event_rx, bash_confirm_rx)
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Create and start the process monitor, bridging ProcessEvent → AgentEvent.
#[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "traversal loop; bounded by config chain length"))]
fn create_process_monitor(agent_tx: tokio::sync::broadcast::Sender<AgentEvent>) -> Arc<crate::procmon::ProcessMonitor> {
    let config = crate::procmon::ProcessMonitorConfig::default();
    let (proc_tx, mut proc_rx) = tokio::sync::broadcast::channel::<crate::procmon::ProcessEvent>(256);
    let monitor = Arc::new(crate::procmon::ProcessMonitor::new(config, Some(proc_tx)));
    monitor.clone().start();

    tokio::spawn(async move {
        loop {
            match proc_rx.recv().await {
                Ok(pe) => {
                    agent_tx.send(crate::agent::events::process_event_to_agent(pe)).ok();
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    });

    monitor
}

/// Populate tool info on the app for the /tools slash command display.
fn populate_tool_info(app: &mut App, tools: &[Arc<dyn crate::tools::Tool>]) {
    let builtin_names: std::collections::HashSet<&str> = [
        "read",
        "write",
        "edit",
        "bash",
        "grep",
        "find",
        "ls",
        "subagent",
        "delegate_task",
        "todo",
        "web",
        "commit",
        "review",
        "ask",
        "image_gen",
        "validate_tui",
    ]
    .into_iter()
    .collect();

    for tool in tools {
        let def = tool.definition();
        let source = if builtin_names.contains(def.name.as_str()) {
            "built-in".to_string()
        } else {
            "plugin".to_string()
        };
        app.tool_info.push((def.name.clone(), def.description.clone(), source));
    }
    app.tool_info.sort_by(|a, b| a.2.cmp(&b.2).then(a.0.cmp(&b.0)));
}

/// Helper to access the ProcessPanel.
#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "panel registered at startup"))]
fn process_panel(app: &mut App) -> &mut crate::tui::components::process_panel::ProcessPanel {
    app.panels
        .downcast_mut::<crate::tui::components::process_panel::ProcessPanel>(crate::tui::panel::PanelId::Processes)
        .expect("process panel registered at startup")
}
