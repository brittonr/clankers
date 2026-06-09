//! Agent construction — build the Agent with all tools wired up.
//!
//! Extracted from interactive.rs to isolate tool construction, process
//! monitor wiring, and agent builder configuration.

use std::sync::Arc;

use clankers_agent::Agent;
use clankers_agent::events::AgentEvent;
use clankers_tui::app::App;

/// Build agent with all tools and configuration.
///
/// Creates the process monitor, wires it into the TUI panel, constructs
/// all tools (filtering disabled ones), and builds the final agent with
/// optional DB, routing, and cost tracking.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(crate) fn build_agent_with_tools(
    provider: Arc<dyn clankers_provider::Provider>,
    settings: &clankers_config::settings::Settings,
    model: String,
    system_prompt: String,
    app: &mut App,
    panel_tx: tokio::sync::mpsc::UnboundedSender<clankers_tui::components::subagent_event::SubagentEvent>,
    todo_tx: tokio::sync::mpsc::UnboundedSender<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    plugin_manager: Option<&Arc<std::sync::Mutex<clankers_plugin::PluginManager>>>,
    paths: &clankers_config::ClankersPaths,
    db: &Option<clankers_db::Db>,
    schedule_engine: Option<Arc<clanker_scheduler::ScheduleEngine>>,
) -> (Agent, tokio::sync::broadcast::Receiver<AgentEvent>, crate::tools::bash::ConfirmRx) {
    let model_service: Arc<dyn clankers_agent::AgentModelService> = Arc::new(
        crate::agent_runtime_adapters::ProviderModelServiceAdapter::new(Arc::clone(&provider)),
    );

    // Create a temporary agent with empty tools to get event_tx for tool construction
    let temp_agent = Agent::new_with_agent_settings(
        Arc::clone(&model_service),
        Vec::new(),
        crate::agent_config::agent_settings_from_config(settings),
        model.clone(),
        system_prompt.clone(),
    );
    let event_tx = temp_agent.event_sender();
    let (bash_confirm_tx, bash_confirm_rx) = crate::tools::bash::confirm_channel();

    // Create and start the process monitor (bridges ProcessEvent → AgentEvent)
    let process_monitor = create_process_monitor(event_tx.clone());

    // Wire process monitor into the TUI panel
    *process_panel(app) = clankers_tui::components::process_panel::ProcessPanel::new()
        .with_monitor(process_monitor.clone() as Arc<dyn clanker_tui_types::ProcessDataSource>);

    let tool_env = crate::modes::common::ToolEnv {
        settings: Some(settings.clone()),
        event_tx: Some(event_tx),
        panel_tx: Some(panel_tx),
        todo_tx: Some(todo_tx),
        bash_confirm_tx: Some(bash_confirm_tx),
        process_monitor: Some(process_monitor),
        actor_ctx: None,
        schedule_engine,
        mcp_registry: None,
    };
    let tiered_tools = crate::modes::common::build_all_tiered_tools(&tool_env, plugin_manager);

    let active_tiers = crate::tool_gateway::standalone_toolsets();
    let tool_set = crate::modes::common::ToolSet::new(tiered_tools.clone(), active_tiers);

    // tool_info shows ALL tools (for toggle menu); active_tools only sends active tiers to agent
    let all_tools = tool_set.all_tools();
    populate_tool_info(app, &all_tools);

    // Fire plugin_init event so plugins can set up their initial UI
    if let Some(pm) = plugin_manager {
        for action in crate::modes::common::fire_plugin_init(pm) {
            clankers_plugin::ui::apply_ui_action(&mut app.plugin_ui, action);
        }
    }

    // Filter out disabled tools from the active tier set
    let active_tools = crate::tool_gateway::allowed_tools_for_policy(&tiered_tools, &active_tiers, &app.disabled_tools);

    // Build the final agent with tools, db, routing, and cost tracking
    let builder_config = crate::agent_config::agent_builder_config_from_settings(
        settings,
        provider.models(),
        Some(&paths.global_config_dir),
    );
    let mut agent_builder = clankers_agent::builder::AgentBuilder::new(model_service, builder_config, model, system_prompt)
        .with_tools(active_tools);

    // Apply default capability restrictions from settings
    if let Some(caps) = &settings.default_capabilities {
        let gate = std::sync::Arc::new(crate::capability_gate::UcanCapabilityGate::new(caps.clone()));
        agent_builder = agent_builder.with_capability_gate(gate);
    }

    // Attach the global database through agent-owned service ports.
    if let Some(db) = db {
        agent_builder = agent_builder
            .with_memory_context_provider(std::sync::Arc::new(
                crate::agent_runtime_adapters::DbMemoryContextProvider::new(db.clone()),
            ))
            .with_tool_context_service(std::sync::Arc::new(db.clone()))
            .with_tool_search_service(std::sync::Arc::new(
                crate::agent_runtime_adapters::DbMemorySearchService::new(db.clone()),
            ));
        let db = db.clone();
        tokio::spawn(async move {
            crate::tools::process::reconcile_durable_native_process_jobs(&db).await;
        });
    }

    // Build the agent (automatically wires routing and cost tracking from settings)
    let agent = agent_builder.build();

    // Extract cost tracker reference for the app UI
    if settings.cost_tracking.is_some() {
        app.cost_tracker = agent.cost_tracker().cloned();
    }

    let event_rx = agent.subscribe();

    (agent, event_rx, bash_confirm_rx)
}

// ── Helpers ──────────────────────────────────────────────────────────

/// Create and start the process monitor, bridging ProcessEvent → AgentEvent.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "traversal loop; bounded by config chain length")
)]
fn create_process_monitor(
    agent_tx: tokio::sync::broadcast::Sender<AgentEvent>,
) -> Arc<clankers_procmon::ProcessMonitor> {
    let config = clankers_procmon::ProcessMonitorConfig::default();
    let (proc_tx, mut proc_rx) = tokio::sync::broadcast::channel::<clankers_procmon::ProcessEvent>(256);
    let monitor = Arc::new(clankers_procmon::ProcessMonitor::new(config, Some(proc_tx)));
    monitor.clone().start();

    tokio::spawn(async move {
        loop {
            match proc_rx.recv().await {
                Ok(pe) => {
                    agent_tx.send(clankers_agent::events::process_event_to_agent(pe)).ok();
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
        "steel_eval",
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
fn process_panel(app: &mut App) -> &mut clankers_tui::components::process_panel::ProcessPanel {
    app.panels
        .downcast_mut::<clankers_tui::components::process_panel::ProcessPanel>(clankers_tui::panel::PanelId::Processes)
        .expect("process panel registered at startup")
}
