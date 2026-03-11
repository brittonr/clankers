//! Plugin system (Extism WASM) — re-exported from `clankers-plugin`.

// Re-export core types from the extracted crate
pub use clankers_plugin::PluginInfo;
pub use clankers_plugin::PluginManager;
pub use clankers_plugin::PluginState;
pub use clankers_plugin::load_plugins_from_dir;

// Re-export sub-modules
pub use clankers_plugin::bridge;
pub use clankers_plugin::hooks;
pub use clankers_plugin::host;
pub use clankers_plugin::manifest;
pub use clankers_plugin::registry;
pub use clankers_plugin::sandbox;
pub use clankers_plugin::ui;

// Contributions stay in the main crate (they impl main-crate traits on PluginManager)
pub mod contributions;

#[cfg(test)]
mod tests;
