//! Daemon tool/plugin projection helpers.
//!
//! Session actors trigger refresh and drain operations, but construction of
//! tool rebuilders and plugin protocol projections lives here so actor loops
//! stay focused on multiplexing commands, signals, and event streams.

use std::sync::Arc;
use std::sync::Mutex;

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
