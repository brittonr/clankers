use std::sync::Arc;
use std::sync::Mutex;

use crate::PluginInfo;
use crate::PluginManager;
use crate::PluginState;
use crate::bridge::PluginEvent;

/// Runtime-facing plugin summary used by the unified host facade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginRuntimeSummary {
    pub name: String,
    pub version: String,
    pub state: String,
    pub kind: String,
    pub tools: Vec<String>,
    pub permissions: Vec<String>,
    pub last_error: Option<String>,
}

impl From<&PluginInfo> for PluginRuntimeSummary {
    fn from(info: &PluginInfo) -> Self {
        Self {
            name: info.name.clone(),
            version: info.version.clone(),
            state: info.state.summary_label().to_string(),
            kind: info.manifest.kind.as_str().to_string(),
            tools: info.declared_tool_inventory(),
            permissions: info.manifest.permissions.clone(),
            last_error: info.state.last_error().map(|error| error.to_string()),
        }
    }
}

/// Shared facade over the current plugin runtime.
///
/// Today this wraps the Extism-backed `PluginManager`. Later tasks can add
/// stdio state behind the same query and dispatch surface.
#[derive(Clone)]
pub struct PluginHostFacade {
    manager: Arc<Mutex<PluginManager>>,
}

impl PluginHostFacade {
    pub fn new(manager: Arc<Mutex<PluginManager>>) -> Self {
        Self { manager }
    }

    pub fn plugin_infos(&self) -> Vec<PluginInfo> {
        let manager = self.manager.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        });
        manager.list().into_iter().cloned().collect()
    }

    pub fn active_plugins(&self) -> Vec<PluginInfo> {
        self.plugin_infos().into_iter().filter(|info| info.state == PluginState::Active).collect()
    }

    pub fn event_subscribers(&self, event_kind: &str) -> Vec<PluginInfo> {
        self.active_plugins()
            .into_iter()
            .filter(|info| info.manifest.kind.uses_wasm_runtime())
            .filter(|info| {
                info.manifest
                    .events
                    .iter()
                    .any(|event| PluginEvent::parse(event).is_some_and(|parsed| parsed.matches_event_kind(event_kind)))
            })
            .collect()
    }

    pub fn stdio_event_subscribers(&self, event_kind: &str) -> Vec<PluginInfo> {
        let manager = self.manager.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        });
        manager
            .list()
            .into_iter()
            .filter(|info| info.manifest.kind == crate::manifest::PluginKind::Stdio)
            .filter(|info| info.state == PluginState::Active)
            .filter(|info| manager.live_event_subscriptions(&info.name).iter().any(|event| event == event_kind))
            .cloned()
            .collect()
    }

    pub fn has_event_subscriber(&self, event_kind: &str) -> bool {
        !self.event_subscribers(event_kind).is_empty()
    }

    pub fn has_function(&self, plugin: &str, function: &str) -> bool {
        let manager = self.manager.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        });
        manager.has_function(plugin, function)
    }

    pub fn call_plugin(&self, plugin: &str, function: &str, input: &str) -> Result<String, String> {
        let manager = self.manager.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        });
        manager.call_plugin(plugin, function, input)
    }

    pub fn summaries(&self) -> Vec<PluginRuntimeSummary> {
        let manager = self.manager.lock().unwrap_or_else(|poisoned| {
            tracing::warn!("Plugin manager mutex was poisoned, recovering");
            poisoned.into_inner()
        });
        manager
            .list()
            .into_iter()
            .map(|info| PluginRuntimeSummary {
                name: info.name.clone(),
                version: info.version.clone(),
                state: info.state.summary_label().to_string(),
                kind: info.manifest.kind.as_str().to_string(),
                tools: if info.manifest.kind.uses_wasm_runtime() {
                    info.declared_tool_inventory()
                } else {
                    manager.live_tool_inventory(&info.name)
                },
                permissions: info.manifest.permissions.clone(),
                last_error: info.state.last_error().map(|error| error.to_string()),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::path::PathBuf;

    use serde_json::json;

    use super::*;

    fn write_plugin_manifest(dir: &Path, name: &str, manifest: serde_json::Value) {
        let plugin_dir = dir.join(name);
        std::fs::create_dir_all(&plugin_dir).unwrap();
        std::fs::write(plugin_dir.join("plugin.json"), serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
    }

    fn build_host_with_plugins() -> PluginHostFacade {
        let dir = std::env::temp_dir().join(format!(
            "clankers-plugin-host-facade-{}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        write_plugin_manifest(
            &dir,
            "event-plugin",
            json!({
                "name": "event-plugin",
                "version": "0.1.0",
                "kind": "extism",
                "wasm": "plugin.wasm",
                "events": ["tool_call"],
                "tools": ["event_tool"]
            }),
        );
        write_plugin_manifest(
            &dir,
            "quiet-plugin",
            json!({
                "name": "quiet-plugin",
                "version": "0.1.0",
                "kind": "extism",
                "wasm": "plugin.wasm",
                "tools": ["quiet_tool"]
            }),
        );
        write_plugin_manifest(
            &dir,
            "broken-plugin",
            json!({
                "name": "broken-plugin",
                "version": "0.1.0",
                "kind": "stdio",
                "stdio": {
                    "args": ["plugin.py"],
                    "sandbox": "inherit"
                }
            }),
        );

        let plugins_dir = PathBuf::from(&dir);
        let mut manager = PluginManager::new(plugins_dir, None);
        manager.discover();
        manager.get_mut("event-plugin").unwrap().state = PluginState::Active;
        manager.get_mut("quiet-plugin").unwrap().state = PluginState::Active;

        let manager = Arc::new(Mutex::new(manager));
        PluginHostFacade::new(manager)
    }

    #[test]
    fn facade_filters_event_subscribers_from_active_plugins() {
        let host = build_host_with_plugins();
        let subscribers = host.event_subscribers("tool_call");
        let names: Vec<String> = subscribers.into_iter().map(|info| info.name).collect();
        assert_eq!(names, vec!["event-plugin"]);
        assert!(host.has_event_subscriber("tool_call"));
        assert!(!host.has_event_subscriber("usage_update"));
    }

    #[test]
    fn facade_summaries_include_kind_and_last_error() {
        let host = build_host_with_plugins();
        let summaries = host.summaries();

        let event = summaries.iter().find(|summary| summary.name == "event-plugin").unwrap();
        assert_eq!(event.kind, "extism");
        assert_eq!(event.state, "Active");
        assert!(event.last_error.is_none());

        let broken = summaries.iter().find(|summary| summary.name == "broken-plugin").unwrap();
        assert_eq!(broken.kind, "stdio");
        assert_eq!(broken.state, "Error");
        assert!(broken.last_error.as_deref().is_some_and(|error| error.contains("stdio.command")));
    }
}
