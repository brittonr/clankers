//! Plugin system (Extism WASM) — re-exported from `clankers-plugin`.

// Re-export core types from the extracted crate
pub use clankers_plugin::PluginHostFacade;
pub use clankers_plugin::PluginInfo;
pub use clankers_plugin::PluginManager;
pub use clankers_plugin::PluginRuntimeSummary;
pub use clankers_plugin::PluginState;
// Re-export sub-modules
pub use clankers_plugin::bridge;
pub use clankers_plugin::hooks;
pub use clankers_plugin::host;
pub use clankers_plugin::load_plugins_from_dir;
pub use clankers_plugin::manifest;
pub use clankers_plugin::registry;
pub use clankers_plugin::sandbox;
pub use clankers_plugin::ui;

// Contributions stay in the main crate (they impl main-crate traits on PluginManager)
pub mod contributions;

pub fn build_protocol_plugin_summaries(
    plugin_manager: &std::sync::Arc<std::sync::Mutex<PluginManager>>,
) -> Vec<clankers_protocol::PluginSummary> {
    PluginHostFacade::new(std::sync::Arc::clone(plugin_manager))
        .summaries()
        .into_iter()
        .map(|summary| clankers_protocol::PluginSummary {
            name: summary.name,
            version: summary.version,
            state: summary.state,
            tools: summary.tools,
            permissions: summary.permissions,
            kind: Some(summary.kind),
            last_error: summary.last_error,
        })
        .collect()
}

#[cfg(test)]
mod tests;
