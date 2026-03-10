use std::path::PathBuf;

use super::*;
use crate::plugin::PluginState;

fn plugins_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins")
}

// ── disable ──────────────────────────────────────────────────────

#[test]
fn disable_active_plugin() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);

    mgr.disable("clankers-test-plugin").expect("disable should succeed");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Disabled);

    // Calling the plugin should fail now
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", "{}");
    assert!(result.is_err(), "Disabled plugin should not be callable");
}

#[test]
fn disable_nonexistent_plugin() {
    let mgr_result = PluginManager::new(plugins_dir(), None);
    let mut mgr = mgr_result;
    let result = mgr.disable("no-such-plugin");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

// ── enable ───────────────────────────────────────────────────────

#[test]
fn enable_disabled_plugin() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    mgr.disable("clankers-test-plugin").expect("disable");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Disabled);

    mgr.enable("clankers-test-plugin").expect("enable should succeed");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);

    // Plugin should be callable again
    let input = serde_json::json!({"tool": "test_echo", "args": {"text": "hello"}});
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", &input.to_string());
    assert!(result.is_ok(), "Re-enabled plugin should work: {:?}", result);
}

#[test]
fn enable_already_active_plugin_fails() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    let result = mgr.enable("clankers-test-plugin");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not disabled"));
}

#[test]
fn enable_nonexistent_plugin() {
    let mut mgr = PluginManager::new(plugins_dir(), None);
    let result = mgr.enable("no-such-plugin");
    assert!(result.is_err());
}

// ── reload ───────────────────────────────────────────────────────

#[test]
fn reload_active_plugin() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    mgr.reload("clankers-test-plugin").expect("reload should succeed");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);

    // Plugin should still work after reload
    let input = serde_json::json!({"tool": "test_echo", "args": {"text": "after reload"}});
    let result = mgr.call_plugin("clankers-test-plugin", "handle_tool_call", &input.to_string());
    assert!(result.is_ok());
}

// ── disabled_plugins ─────────────────────────────────────────────

#[test]
fn disabled_plugins_list() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    assert!(mgr.disabled_plugins().is_empty());

    mgr.disable("clankers-test-plugin").expect("disable");
    let disabled = mgr.disabled_plugins();
    assert_eq!(disabled.len(), 1);
    assert!(disabled.contains(&"clankers-test-plugin".to_string()));
}

// ── apply_disabled_set ───────────────────────────────────────────

#[test]
fn apply_disabled_set_disables_matching_plugins() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");

    let disabled = vec!["clankers-test-plugin".to_string(), "nonexistent-plugin".to_string()];
    mgr.apply_disabled_set(&disabled);

    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Disabled);
}

// ── reload_all skips disabled ────────────────────────────────────

#[test]
fn reload_all_skips_disabled() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");
    mgr.disable("clankers-test-plugin").expect("disable");

    mgr.reload_all();

    // Should still be disabled after reload_all
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Disabled);
}

// ── disable/enable round-trip ────────────────────────────────────

#[test]
fn disable_enable_round_trip() {
    let mut mgr = manager_with_test_plugin();
    mgr.load_wasm("clankers-test-plugin").expect("load");

    for _ in 0..3 {
        mgr.disable("clankers-test-plugin").expect("disable");
        assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Disabled);
        assert!(mgr.call_plugin("clankers-test-plugin", "handle_tool_call", "{}").is_err());

        mgr.enable("clankers-test-plugin").expect("enable");
        assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);

        let input = serde_json::json!({"tool": "test_echo", "args": {"text": "cycle"}});
        assert!(mgr.call_plugin("clankers-test-plugin", "handle_tool_call", &input.to_string()).is_ok());
    }
}
