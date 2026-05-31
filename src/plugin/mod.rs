//! Main-crate plugin adapters that cannot live in `clankers-plugin`.
//!
//! Runtime/plugin types are imported from `clankers-plugin` directly by call sites.
//! This module keeps only main-crate glue: trait contributions and protocol summary
//! projection.

// Contributions stay in the main crate (they impl main-crate traits on PluginManager)
pub mod contributions;

pub fn build_protocol_plugin_summaries(
    plugin_manager: &std::sync::Arc<std::sync::Mutex<clankers_plugin::PluginManager>>,
) -> Vec<clankers_protocol::PluginSummary> {
    clankers_plugin::PluginHostFacade::new(std::sync::Arc::clone(plugin_manager))
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
pub(crate) mod tests;
