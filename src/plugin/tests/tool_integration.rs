use super::*;

// ── Plugin tool integration tests ────────────────────────────────

#[test]
fn build_plugin_tools_creates_tools_from_definitions() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);

    // Should have tools from all discovered plugins (test-plugin + self-validate)
    assert!(tools.len() >= 2, "Expected at least 2 plugin tools, got {}", tools.len());

    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    assert!(names.contains(&"test_echo".to_string()));
    assert!(names.contains(&"test_reverse".to_string()));

    // Verify descriptions come from tool_definitions
    let echo = tools.iter().find(|t| t.definition().name == "test_echo").unwrap();
    assert!(echo.definition().description.contains("Echo"), "desc: {}", echo.definition().description);
}

#[test]
fn build_all_tools_includes_plugin_tools() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
    let env = crate::modes::common::ToolEnv::default();
    let tools = crate::modes::common::build_all_tools_with_env(&env, Some(&manager));

    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    // Built-in tools
    assert!(names.contains(&"read".to_string()));
    assert!(names.contains(&"bash".to_string()));
    // Plugin tools
    assert!(names.contains(&"test_echo".to_string()));
    assert!(names.contains(&"test_reverse".to_string()));
}

#[test]
fn build_plugin_tools_empty_when_no_plugins() {
    let dir = tempfile::tempdir().unwrap();
    let manager = crate::modes::common::init_plugin_manager(dir.path(), None, &[]);
    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);
    assert!(tools.is_empty());
}

#[test]
fn build_plugin_tools_includes_hash_tools() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);

    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    assert!(names.contains(&"hash_text".to_string()), "Should have hash_text tool");
    assert!(names.contains(&"encode_text".to_string()), "Should have encode_text tool");
}

#[test]
fn build_plugin_tools_includes_email_tools() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);

    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    assert!(names.contains(&"send_email".to_string()), "Should have send_email tool, got: {:?}", names);
    assert!(names.contains(&"list_mailboxes".to_string()), "Should have list_mailboxes tool, got: {:?}", names);
}

#[test]
fn build_plugin_tools_includes_github_tools() {
    let plugins_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("plugins");
    let manager = crate::modes::common::init_plugin_manager(&plugins_dir, None, &[]);
    let tools = crate::modes::common::build_plugin_tools(&[], &manager, None);

    let names: Vec<String> = tools.iter().map(|t| t.definition().name.clone()).collect();
    assert!(names.contains(&"github_pr_list".to_string()), "Should have github_pr_list tool, got: {:?}", names);
    assert!(names.contains(&"github_pr_get".to_string()), "Should have github_pr_get tool");
    assert!(names.contains(&"github_pr_create".to_string()), "Should have github_pr_create tool");
    assert!(names.contains(&"github_issues".to_string()), "Should have github_issues tool");
    assert!(names.contains(&"github_issue_get".to_string()), "Should have github_issue_get tool");
    assert!(names.contains(&"github_actions_status".to_string()), "Should have github_actions_status tool");
    assert!(names.contains(&"github_repo_info".to_string()), "Should have github_repo_info tool");
}
