use super::*;

// ── WASM loading tests ───────────────────────────────────────────

#[test]
fn load_wasm_transitions_to_active() {
    let mgr = loaded_manager();
    let info = mgr.get("clankers-test-plugin").unwrap();
    assert_eq!(info.state, PluginState::Active);
}

#[test]
fn load_wasm_unknown_plugin_errors() {
    let mut mgr = manager_with_test_plugin();
    let result = mgr.load_wasm("nonexistent-plugin");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("not found"));
}

#[test]
fn load_wasm_missing_file_errors() {
    let dir = tempfile::tempdir().unwrap();
    let plugin_dir = dir.path().join("broken-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(plugin_dir.join("plugin.json"), r#"{"name":"broken-plugin","version":"0.1.0"}"#).ok();

    let mut mgr = PluginManager::new(dir.path().to_path_buf(), None);
    mgr.discover();
    let result = mgr.load_wasm("broken-plugin");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("WASM file not found"));
}

// ── Function existence checks ────────────────────────────────────

#[test]
fn has_function_exported_functions() {
    let mgr = loaded_manager();
    assert!(mgr.has_function("clankers-test-plugin", "echo"));
    assert!(mgr.has_function("clankers-test-plugin", "greet"));
    assert!(mgr.has_function("clankers-test-plugin", "reverse"));
    assert!(mgr.has_function("clankers-test-plugin", "length"));
    assert!(mgr.has_function("clankers-test-plugin", "handle_tool_call"));
    assert!(mgr.has_function("clankers-test-plugin", "on_event"));
    assert!(mgr.has_function("clankers-test-plugin", "describe"));
    assert!(mgr.has_function("clankers-test-plugin", "count_words"));
    assert!(mgr.has_function("clankers-test-plugin", "uppercase"));
    assert!(mgr.has_function("clankers-test-plugin", "fail"));
    assert!(mgr.has_function("clankers-test-plugin", "parse_json"));
}

#[test]
fn has_function_nonexistent_returns_false() {
    let mgr = loaded_manager();
    assert!(!mgr.has_function("clankers-test-plugin", "nonexistent_function"));
    assert!(!mgr.has_function("clankers-test-plugin", ""));
}

#[test]
fn has_function_unloaded_plugin_returns_false() {
    let mgr = manager_with_test_plugin();
    assert!(!mgr.has_function("clankers-test-plugin", "echo"));
}

// ── Reload ───────────────────────────────────────────────────────

#[test]
fn reload_plugin() {
    let mut mgr = loaded_manager();
    // Call once before reload
    let r1 = mgr.call_plugin("clankers-test-plugin", "echo", "before").unwrap();
    assert_eq!(r1, "before");

    // Reload
    mgr.reload("clankers-test-plugin").unwrap();

    // Should still work after reload
    let r2 = mgr.call_plugin("clankers-test-plugin", "echo", "after").unwrap();
    assert_eq!(r2, "after");
    assert_eq!(mgr.get("clankers-test-plugin").unwrap().state, PluginState::Active);
}

#[test]
fn reload_all_plugins() {
    let mut mgr = loaded_manager();
    mgr.reload_all();
    // Should still function
    let result = mgr.call_plugin("clankers-test-plugin", "echo", "test").unwrap();
    assert_eq!(result, "test");
}

// ── Multiple calls (statefulness check) ──────────────────────────

#[test]
fn multiple_sequential_calls() {
    let mgr = loaded_manager();
    for i in 0..100 {
        let input = format!("call-{i}");
        let result = mgr.call_plugin("clankers-test-plugin", "echo", &input).unwrap();
        assert_eq!(result, input);
    }
}
