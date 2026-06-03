//! Daemon tool/plugin projection helpers.
//!
//! Session actors trigger a tick service, but construction of tool rebuilders,
//! plugin protocol projections, and periodic drain policy lives here so actor
//! loops stay focused on multiplexing commands, signals, and event streams.

use std::sync::Arc;
use std::sync::Mutex;

use clanker_tui_types::SubagentEvent;
use clankers_controller::SessionController;
use clankers_protocol::DaemonEvent;
use clankers_protocol::PluginSummary;
use tokio::sync::broadcast;

use super::socket_bridge::SessionFactory;

#[derive(Clone)]
pub(crate) struct DaemonPluginProjection {
    plugin_manager: Option<Arc<Mutex<clankers_plugin::PluginManager>>>,
}

impl DaemonPluginProjection {
    pub(crate) fn new(plugin_manager: Option<Arc<Mutex<clankers_plugin::PluginManager>>>) -> Self {
        Self { plugin_manager }
    }

    pub(crate) fn manager(&self) -> Option<&Arc<Mutex<clankers_plugin::PluginManager>>> {
        self.plugin_manager.as_ref()
    }

    pub(crate) fn summaries(&self) -> Vec<PluginSummary> {
        let Some(pm) = self.manager() else {
            return Vec::new();
        };
        crate::plugin::build_protocol_plugin_summaries(pm)
    }

    pub(crate) fn fire_init(&self, event_tx: &broadcast::Sender<DaemonEvent>) {
        let Some(pm) = self.manager() else {
            return;
        };
        for action in crate::modes::common::fire_plugin_init(pm) {
            event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
        }
    }

    pub(crate) fn drain_runtime_events(&self, event_tx: &broadcast::Sender<DaemonEvent>) {
        drain_plugin_runtime_events(event_tx, self.manager());
    }
}

/// Actor-loop tick service for daemon session background projections.
///
/// The actor owns multiplexing. This service owns the periodic and
/// after-command projection steps: tool inventory refresh, controller event
/// broadcast, and asynchronous plugin runtime UI drains.
#[derive(Clone)]
pub(crate) struct DaemonSessionTickService {
    plugin_projection: DaemonPluginProjection,
}

impl DaemonSessionTickService {
    pub(crate) fn new(plugin_projection: DaemonPluginProjection) -> Self {
        Self { plugin_projection }
    }

    pub(crate) fn for_plugin_manager(plugin_manager: Option<Arc<Mutex<clankers_plugin::PluginManager>>>) -> Self {
        Self::new(DaemonPluginProjection::new(plugin_manager))
    }

    pub(crate) fn plugin_manager(&self) -> Option<&Arc<Mutex<clankers_plugin::PluginManager>>> {
        self.plugin_projection.manager()
    }

    pub(crate) fn plugin_summaries(&self) -> Vec<PluginSummary> {
        self.plugin_projection.summaries()
    }

    pub(crate) fn start_session(&self, event_tx: &broadcast::Sender<DaemonEvent>) {
        self.plugin_projection.fire_init(event_tx);
        self.plugin_projection.drain_runtime_events(event_tx);
    }

    pub(crate) fn drain_background(
        &self,
        controller: &mut SessionController,
        event_tx: &broadcast::Sender<DaemonEvent>,
        panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SubagentEvent>,
    ) {
        sync_tool_inventory(controller, event_tx);
        self.drain_controller_and_plugins(controller, event_tx, panel_rx);
    }

    pub(crate) fn drain_after_command(
        &self,
        controller: &mut SessionController,
        event_tx: &broadcast::Sender<DaemonEvent>,
        panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SubagentEvent>,
        tools_before: Vec<clankers_protocol::ToolInfo>,
    ) {
        self.drain_controller_and_plugins(controller, event_tx, panel_rx);
        let tools_after = controller.current_tool_infos();
        if tools_after != tools_before {
            event_tx.send(DaemonEvent::ToolList { tools: tools_after }).ok();
        }
    }

    pub(crate) fn drain_controller_and_plugins(
        &self,
        controller: &mut SessionController,
        event_tx: &broadcast::Sender<DaemonEvent>,
        panel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<SubagentEvent>,
    ) {
        super::socket_bridge::drain_and_broadcast(controller, event_tx, panel_rx, self.plugin_manager());
        self.plugin_projection.drain_runtime_events(event_tx);
    }
}

/// Tool rebuilder that uses the daemon's SessionFactory to rebuild
/// the filtered tool set when disabled tools change.
pub(crate) struct DaemonToolRebuilder {
    factory: Arc<SessionFactory>,
}

impl DaemonToolRebuilder {
    pub(crate) fn new(factory: Arc<SessionFactory>) -> Self {
        Self { factory }
    }
}

impl clankers_controller::ToolRebuilder for DaemonToolRebuilder {
    fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn crate::tools::Tool>> {
        let disabled_set: std::collections::HashSet<String> = disabled.iter().cloned().collect();
        // Build a fresh panel_tx (events go nowhere — we only need the tool list)
        let (panel_tx, _) = tokio::sync::mpsc::unbounded_channel();
        let child_factory = self.factory.child_actor_factory();
        let actor_ctx = self.factory.registry.as_ref().zip(child_factory).map(|(registry, factory)| {
            crate::tools::subagent::ActorContext {
                registry: registry.clone(),
                factory,
            }
        });
        let env = crate::modes::common::ToolEnv {
            settings: Some(self.factory.settings.clone()),
            panel_tx: Some(panel_tx),
            actor_ctx,
            schedule_engine: self.factory.schedule_engine.clone(),
            ..Default::default()
        };
        let tiered = crate::modes::common::build_all_tiered_tools(&env, self.factory.plugin_manager.as_ref());
        crate::tool_gateway::allowed_tools_for_policy(&tiered, &crate::tool_gateway::daemon_toolsets(), &disabled_set)
    }
}

pub(crate) fn tool_rebuilder_for_factory(factory: Arc<SessionFactory>) -> Arc<dyn clankers_controller::ToolRebuilder> {
    Arc::new(DaemonToolRebuilder::new(factory))
}

pub(crate) fn sync_tool_inventory(controller: &mut SessionController, event_tx: &broadcast::Sender<DaemonEvent>) {
    if controller.refresh_tools() {
        event_tx
            .send(DaemonEvent::ToolList {
                tools: controller.current_tool_infos(),
            })
            .ok();
    }
}

pub(crate) fn drain_plugin_runtime_events(
    event_tx: &broadcast::Sender<DaemonEvent>,
    plugin_manager: Option<&Arc<Mutex<clankers_plugin::PluginManager>>>,
) {
    let Some(pm) = plugin_manager else {
        return;
    };

    let result = crate::modes::plugin_dispatch::drain_stdio_runtime_outputs(pm);
    for (plugin_name, message) in result.messages {
        event_tx
            .send(DaemonEvent::SystemMessage {
                text: format!("🔌 {}: {}", plugin_name, message),
                is_error: false,
            })
            .ok();
    }
    for action in result.ui_actions {
        event_tx.send(crate::modes::plugin_dispatch::ui_action_to_daemon_event(action)).ok();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clankers_controller::ToolRebuilder;
    use clankers_controller::config::ControllerConfig;
    use tokio::sync::broadcast;
    use tokio::sync::mpsc;

    use super::*;

    struct StubProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for StubProvider {
        async fn complete(
            &self,
            _req: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clanker_message::streaming::StreamEvent>,
        ) -> std::result::Result<(), clankers_provider::error::ProviderError> {
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "stub"
        }
    }

    struct StubRebuilder {
        tools: Vec<Arc<dyn crate::tools::Tool>>,
    }

    impl ToolRebuilder for StubRebuilder {
        fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn crate::tools::Tool>> {
            self.tools
                .iter()
                .filter(|tool| !disabled.iter().any(|name| name == &tool.definition().name))
                .cloned()
                .collect()
        }
    }

    struct StubTool {
        definition: crate::tools::ToolDefinition,
        source: &'static str,
    }

    #[async_trait::async_trait]
    impl crate::tools::Tool for StubTool {
        fn definition(&self) -> &crate::tools::ToolDefinition {
            &self.definition
        }

        async fn execute(
            &self,
            _ctx: &crate::tools::ToolContext,
            _params: serde_json::Value,
        ) -> crate::tools::ToolResult {
            crate::tools::ToolResult::text("stub")
        }

        fn source(&self) -> &str {
            self.source
        }
    }

    fn stub_tool(name: &str, source: &'static str) -> Arc<dyn crate::tools::Tool> {
        Arc::new(StubTool {
            definition: crate::tools::ToolDefinition {
                name: name.to_string(),
                description: format!("stub {name}"),
                input_schema: serde_json::json!({"type": "object"}),
            },
            source,
        })
    }

    fn test_controller() -> SessionController {
        let model = "test-model".to_string();
        let agent = clankers_agent::builder::AgentBuilder::new(
            Arc::new(StubProvider),
            clankers_agent::builder::AgentBuilderConfig::default(),
            model.clone(),
            String::new(),
        )
        .build();
        SessionController::new(agent, ControllerConfig {
            session_id: "tick-service-test".to_string(),
            model,
            ..Default::default()
        })
    }

    #[tokio::test]
    async fn tick_service_refreshes_tool_inventory_without_socket() {
        let mut controller = test_controller();
        controller.set_tool_rebuilder(Arc::new(StubRebuilder {
            tools: vec![stub_tool("tick_stdio_tool", "stdio-plugin")],
        }));
        let tick_service = DaemonSessionTickService::for_plugin_manager(None);
        let (event_tx, mut event_rx) = broadcast::channel(8);
        let (_panel_tx, mut panel_rx) = mpsc::unbounded_channel();

        tick_service.drain_background(&mut controller, &event_tx, &mut panel_rx);

        let event = event_rx.try_recv().expect("tool refresh should broadcast one event");
        let DaemonEvent::ToolList { tools } = event else {
            panic!("expected ToolList from tick service");
        };
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "tick_stdio_tool");
        assert_eq!(tools[0].source, "stdio-plugin");

        tick_service.drain_background(&mut controller, &event_tx, &mut panel_rx);
        assert!(event_rx.try_recv().is_err(), "unchanged inventory should not rebroadcast");
    }

    #[tokio::test]
    async fn tick_service_reports_tool_changes_after_controller_command_without_socket() {
        let mut controller = test_controller();
        controller.set_tool_rebuilder(Arc::new(StubRebuilder {
            tools: vec![stub_tool("after_command_tool", "stdio-plugin")],
        }));
        assert!(controller.refresh_tools());
        let tools_before = Vec::new();
        let tick_service = DaemonSessionTickService::for_plugin_manager(None);
        let (event_tx, mut event_rx) = broadcast::channel(8);
        let (_panel_tx, mut panel_rx) = mpsc::unbounded_channel();

        tick_service.drain_after_command(&mut controller, &event_tx, &mut panel_rx, tools_before);

        let event = event_rx.try_recv().expect("post-command tool change should broadcast");
        let DaemonEvent::ToolList { tools } = event else {
            panic!("expected post-command ToolList from tick service");
        };
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "after_command_tool");
    }
}
