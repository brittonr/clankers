use super::*;

// ── GitHub plugin discovery and loading ─────────────────────────

#[test]
fn discover_finds_github_plugin() {
    let mgr = manager_with_test_plugin();
    assert!(mgr.get("clankers-github").is_some(), "GitHub plugin should be discovered");
}

#[test]
fn discover_reads_github_manifest_metadata() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    assert_eq!(info.name, "clankers-github");
    assert_eq!(info.version, "0.1.0");
    assert_eq!(info.state, PluginState::Loaded);
    assert!(info.manifest.description.contains("GitHub"));
}

#[test]
fn discover_reads_github_manifest_tools() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    assert!(info.manifest.tools.contains(&"github_pr_list".to_string()));
    assert!(info.manifest.tools.contains(&"github_pr_get".to_string()));
    assert!(info.manifest.tools.contains(&"github_pr_create".to_string()));
    assert!(info.manifest.tools.contains(&"github_issues".to_string()));
    assert!(info.manifest.tools.contains(&"github_issue_get".to_string()));
    assert!(info.manifest.tools.contains(&"github_actions_status".to_string()));
    assert!(info.manifest.tools.contains(&"github_repo_info".to_string()));
}

#[test]
fn github_plugin_has_net_permission() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    assert!(info.manifest.permissions.contains(&"net".to_string()));
}

#[test]
fn github_plugin_has_allowed_hosts() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    let hosts = info.manifest.allowed_hosts.as_ref().expect("should have allowed_hosts");
    assert!(hosts.contains(&"api.github.com".to_string()));
}

#[test]
fn github_plugin_has_config_env() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    assert_eq!(info.manifest.config_env.get("github_token").map(|s| s.as_str()), Some("GITHUB_TOKEN"));
}

#[test]
fn github_plugin_has_leader_menu() {
    let mgr = manager_with_test_plugin();
    let info = mgr.get("clankers-github").unwrap();
    assert!(!info.manifest.leader_menu.is_empty());
    assert_eq!(info.manifest.leader_menu[0].key, 'G');
    assert_eq!(info.manifest.leader_menu[0].label, "GitHub");
}

#[test]
fn load_github_wasm_transitions_to_active() {
    let mgr = loaded_github_manager();
    let info = mgr.get("clankers-github").unwrap();
    assert_eq!(info.state, PluginState::Active);
}

#[test]
fn github_plugin_has_expected_functions() {
    let mgr = loaded_github_manager();
    assert!(mgr.has_function("clankers-github", "handle_tool_call"));
    assert!(mgr.has_function("clankers-github", "on_event"));
    assert!(mgr.has_function("clankers-github", "describe"));
}

#[test]
fn github_describe_metadata() {
    let mgr = loaded_github_manager();
    let result = mgr.call_plugin("clankers-github", "describe", "null").unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["name"], "clankers-github");
    assert_eq!(parsed["version"], "0.1.0");
    let tools = parsed["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 7);
    let tool_names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(tool_names.contains(&"github_pr_list"));
    assert!(tool_names.contains(&"github_pr_get"));
    assert!(tool_names.contains(&"github_pr_create"));
    assert!(tool_names.contains(&"github_issues"));
    assert!(tool_names.contains(&"github_issue_get"));
    assert!(tool_names.contains(&"github_actions_status"));
    assert!(tool_names.contains(&"github_repo_info"));
}

#[test]
fn github_on_event_agent_start() {
    let mgr = loaded_github_manager();
    let input = r#"{"event":"agent_start","data":{}}"#;
    let result = mgr.call_plugin("clankers-github", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
    assert!(parsed["message"].as_str().unwrap().contains("ready"));
}

#[test]
fn github_on_event_agent_end() {
    let mgr = loaded_github_manager();
    let input = r#"{"event":"agent_end","data":{}}"#;
    let result = mgr.call_plugin("clankers-github", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], true);
}

#[test]
fn github_on_event_unknown() {
    let mgr = loaded_github_manager();
    let input = r#"{"event":"custom_event","data":{}}"#;
    let result = mgr.call_plugin("clankers-github", "on_event", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["handled"], false);
}

#[test]
fn github_unknown_tool_returns_unknown_status() {
    let mgr = loaded_github_manager();
    let input = r#"{"tool":"nonexistent","args":{}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_eq!(parsed["status"], "unknown_tool");
}

#[test]
fn github_pr_list_without_token_returns_config_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_pr_list","args":{"owner":"test","repo":"test"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("GITHUB_TOKEN"), "Error should mention missing token: {}", result_text);
}

#[test]
fn github_pr_list_missing_owner_returns_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_pr_list","args":{"repo":"test"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("owner"), "Error should mention missing owner: {}", result_text);
}

#[test]
fn github_pr_list_missing_repo_returns_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_pr_list","args":{"owner":"test"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("repo"), "Error should mention missing repo: {}", result_text);
}

#[test]
fn github_pr_get_missing_number_returns_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_pr_get","args":{"owner":"test","repo":"test"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("number"), "Error should mention missing number: {}", result_text);
}

#[test]
fn github_pr_create_missing_title_returns_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_pr_create","args":{"owner":"test","repo":"test","head":"feature"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
    let result_text = parsed["result"].as_str().unwrap_or("");
    assert!(result_text.contains("title"), "Error should mention missing title: {}", result_text);
}

#[test]
fn github_issue_get_missing_number_returns_error() {
    let mgr = load_github_plugin_no_config();
    let input = r#"{"tool":"github_issue_get","args":{"owner":"test","repo":"test"}}"#;
    let result = mgr.call_plugin("clankers-github", "handle_tool_call", input).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    assert_ne!(parsed["status"], "ok");
}

// ── GitHub plugin leader menu contribution ──────────────────────

#[test]
fn github_plugin_contributes_leader_menu() {
    use crate::tui::components::leader_menu::MenuContributor;

    let mgr = manager_with_test_plugin();
    let items = mgr.menu_items();

    let github_items: Vec<_> = items.iter().filter(|i| i.source == "clankers-github").collect();
    assert!(!github_items.is_empty(), "GitHub plugin should contribute leader menu items");
    assert_eq!(github_items[0].key, 'G');
    assert_eq!(github_items[0].label, "GitHub");
}
