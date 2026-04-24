use super::*;

// ── Email plugin discovery and loading ──────────────────────────

#[test]
fn discover_finds_email_plugin() {
    let mgr = manager_with_test_plugin();
    assert!(mgr.get("clankers-email").is_some(), "Email plugin should be discovered");
}

#[test]
fn email_plugin_manifest_has_net_permission() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-email").unwrap();
    assert!(info.manifest.permissions.contains(&"net".to_string()));
}

#[test]
fn email_plugin_manifest_has_allowed_hosts() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-email").unwrap();
    let hosts = info.manifest.allowed_hosts.as_ref().expect("should have allowed_hosts");
    assert!(hosts.contains(&"api.fastmail.com".to_string()));
}

#[test]
fn email_plugin_manifest_has_config_env() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-email").unwrap();
    assert_eq!(info.manifest.config_env.get("jmap_token").map(|s| s.as_str()), Some("FASTMAIL_API_TOKEN"));
    assert_eq!(info.manifest.config_env.get("default_from").map(|s| s.as_str()), Some("CLANKERS_EMAIL_FROM"));
}

#[test]
fn load_email_wasm_transitions_to_active() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-email").expect("Failed to load email plugin WASM");
    let info = mgr.get("clankers-email").unwrap();
    assert_eq!(info.state, PluginState::Active);
}

#[test]
fn email_plugin_has_expected_functions() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-email").expect("Failed to load email plugin WASM");
    assert!(mgr.has_function("clankers-email", "handle_tool_call"));
    assert!(mgr.has_function("clankers-email", "on_event"));
    assert!(mgr.has_function("clankers-email", "describe"));
}

#[test]
fn email_plugin_describe() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-email").expect("load");
    let result = mgr.call_plugin("clankers-email", "describe", "null").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["name"], "clankers-email");
    assert_eq!(parsed["version"], "0.2.0");
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 4);
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"send_email"), "missing send_email: {:?}", tool_names);
    assert!(tool_names.contains(&"search_email"), "missing search_email: {:?}", tool_names);
    assert!(tool_names.contains(&"read_email"), "missing read_email: {:?}", tool_names);
    assert!(tool_names.contains(&"list_mailboxes"), "missing list_mailboxes: {:?}", tool_names);
}

#[test]
fn email_plugin_on_event_agent_start() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let mut mgr = PluginManager::new(plugins_dir, None);
    mgr.discover();
    mgr.load_wasm("clankers-email").expect("load");
    let input = r#"{"event":"agent_start","data":{}}"#;
    let result = mgr.call_plugin("clankers-email", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert!(parsed["message"].as_str().unwrap().contains("JMAP"));
}

#[test]
fn email_send_without_config_rejects() {
    let mgr = load_email_plugin_no_config();
    let input =
        r#"{"tool":"send_email","args":{"to":"test@example.com","subject":"Test","body":"Hello","from":"x@x.com"}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok", "Should fail without config: {:?}", parsed);
    // First check hit is the allowlist (no config = no allowlist)
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("allowlist"), "Error should mention missing allowlist: {}", result_text);
}

#[test]
fn email_send_without_from_rejects() {
    let mgr = load_email_plugin_no_config();
    // No "from" param and no CLANKERS_EMAIL_FROM config — hits from check before allowlist
    let input = r#"{"tool":"send_email","args":{"to":"test@example.com","subject":"Test","body":"Hello"}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(
        result_text.contains("from") || result_text.contains("CLANKERS_EMAIL_FROM"),
        "Error should mention missing from: {}",
        result_text
    );
}

#[test]
fn email_list_mailboxes_without_token_returns_config_error() {
    let mgr = load_email_plugin_no_config();
    let input = r#"{"tool":"list_mailboxes","args":{}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
}

#[test]
fn email_search_without_token_returns_config_error() {
    let mgr = load_email_plugin_no_config();
    let input = r#"{"tool":"search_email","args":{}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(
        result_text.contains("jmap_token") || result_text.contains("Missing config"),
        "Error should mention missing config: {}",
        result_text
    );
}

#[test]
fn email_read_without_token_returns_config_error() {
    let mgr = load_email_plugin_no_config();
    let input = r#"{"tool":"read_email","args":{"id":"test-123"}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(
        result_text.contains("jmap_token") || result_text.contains("Missing config"),
        "Error should mention missing config: {}",
        result_text
    );
}

#[test]
fn email_read_without_id_returns_error() {
    let mgr = load_email_plugin_no_config();
    let input = r#"{"tool":"read_email","args":{}}"#;
    let result = mgr.call_plugin("clankers-email", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
}
