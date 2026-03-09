use std::path::PathBuf;

use super::*;

/// Path to the pre-built test plugin directory.
/// The WASM must be built first via `plugins/clankers-test-plugin/build.sh`.
pub fn test_plugin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins/clankers-test-plugin")
}

/// Helper: create a PluginManager pointing at the test plugin directory.
pub fn manager_with_test_plugin() -> PluginManager {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr
}

/// Helper: create a manager and load the test plugin WASM.
pub fn loaded_manager() -> PluginManager {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("Failed to load test plugin WASM");
    mgr
}

/// Helper: create a manager and load the hash plugin WASM.
pub fn loaded_hash_manager() -> PluginManager {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-hash").expect("Failed to load hash plugin WASM");
    mgr
}

/// Helper: create a manager and load the GitHub plugin WASM.
pub fn loaded_github_manager() -> PluginManager {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-github").expect("Failed to load GitHub plugin WASM");
    mgr
}

/// Load the email plugin WASM directly with no config injected,
/// bypassing PluginManager::load_wasm's config_env resolution.
pub fn load_email_plugin_no_config() -> PluginManager {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let wasm_path = plugins_dir.join("clankers-email/clankers_email.wasm");

    // Create bare Extism manifest — no config, no allowed_hosts
    let manifest = extism::Manifest::new([extism::Wasm::file(&wasm_path)]);
    let plugin = extism::Plugin::new(manifest, [], true).expect("load WASM");

    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.instances.insert("clankers-email".to_string(), Mutex::new(plugin));
    if let Some(info) = mgr.plugins.get_mut("clankers-email") {
        info.state = PluginState::Active;
    }
    mgr
}

/// Load GitHub plugin with no config (no GITHUB_TOKEN).
pub fn load_github_plugin_no_config() -> PluginManager {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let wasm_path = plugins_dir.join("clankers-github/clankers_github.wasm");
    let manifest = extism::Manifest::new([extism::Wasm::file(&wasm_path)]);
    let plugin = extism::Plugin::new(manifest, [], true).expect("load WASM");

    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.instances.insert("clankers-github".to_string(), Mutex::new(plugin));
    if let Some(info) = mgr.plugins.get_mut("clankers-github") {
        info.state = PluginState::Active;
    }
    mgr
}

// Test modules
mod bridge;
mod discovery;
mod email_plugin;
mod events;
mod function_calls;
mod github_plugin;
mod hash_plugin;
mod registry;
mod sandbox;
mod test_plugin;
mod tool_integration;
mod ui;
mod wasm_loading;
