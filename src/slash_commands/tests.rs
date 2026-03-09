use super::*;

#[test]
fn test_parse_command_basic() {
    let (cmd, args) = parse_command("/help").expect("failed to parse /help");
    assert_eq!(cmd, "help");
    assert_eq!(args, "");
}

#[test]
fn test_parse_command_with_args() {
    let (cmd, args) = parse_command("/model claude-3-5-sonnet").expect("failed to parse /model");
    assert_eq!(cmd, "model");
    assert_eq!(args, "claude-3-5-sonnet");
}

#[test]
fn test_parse_command_unknown_falls_through_to_prompt_template() {
    // Unknown commands now fall through to the prompt template system
    let result = parse_command("/nonexistent");
    assert!(result.is_some());
    let (cmd, _args) = result.expect("should have parsed unknown command");
    assert_eq!(cmd, "nonexistent");
}

#[test]
fn test_parse_command_invalid_chars_returns_none() {
    // Commands with invalid characters should still return None
    assert!(parse_command("/").is_none());
}

#[test]
fn test_parse_not_slash() {
    assert!(parse_command("hello").is_none());
}

#[test]
fn test_completions_partial() {
    let results = completions("/he");
    assert!(results.iter().any(|c| c.display == "help"), "results: {:?}", results);
}

#[test]
fn test_completions_empty_slash() {
    let results = completions("/");
    assert!(results.len() > 5); // Should return all commands
}

#[test]
fn test_completions_with_space() {
    let results = completions("/model ");
    assert!(results.is_empty()); // Command complete, no more suggestions
}

#[test]
fn test_help_text_not_empty() {
    let text = help_text();
    assert!(text.contains("/help"));
    assert!(text.contains("/clear"));
}

#[test]
fn test_parse_login_no_args() {
    let (cmd, args) = parse_command("/login").expect("failed to parse /login");
    assert_eq!(cmd, "login");
    assert_eq!(args, "");
}

#[test]
fn test_parse_login_with_code() {
    let (cmd, args) = parse_command("/login abc123#state456").expect("failed to parse /login with code");
    assert_eq!(cmd, "login");
    assert_eq!(args, "abc123#state456");
}

#[test]
fn test_completions_login() {
    let results = completions("/lo");
    assert!(results.iter().any(|c| c.display == "login"), "results: {:?}", results);
}

#[test]
fn test_help_text_includes_login() {
    let text = help_text();
    assert!(text.contains("/login"));
}

#[test]
fn test_parse_worker_no_args() {
    let (cmd, args) = parse_command("/worker").expect("failed to parse /worker");
    assert_eq!(cmd, "worker");
    assert_eq!(args, "");
}

#[test]
fn test_parse_worker_with_name_and_task() {
    let (cmd, args) = parse_command("/worker builder fix the tests").expect("failed to parse /worker with args");
    assert_eq!(cmd, "worker");
    assert_eq!(args, "builder fix the tests");
}

#[test]
fn test_parse_share() {
    let (cmd, args) = parse_command("/share").expect("failed to parse /share");
    assert_eq!(cmd, "share");
    assert_eq!(args, "");
}

#[test]
fn test_parse_share_read_only() {
    let (cmd, args) = parse_command("/share --read-only").expect("failed to parse /share with flag");
    assert_eq!(cmd, "share");
    assert_eq!(args, "--read-only");
}

#[test]
fn test_completions_worker() {
    let results = completions("/wo");
    assert!(results.iter().any(|c| c.display == "worker"), "results: {:?}", results);
}

#[test]
fn test_completions_share() {
    let results = completions("/sh");
    assert!(
        results.iter().any(|c| c.display == "share") || results.iter().any(|c| c.display == "shell"),
        "results: {:?}",
        results
    );
}

#[test]
fn test_help_text_includes_worker_and_share() {
    let text = help_text();
    assert!(text.contains("/worker"));
    assert!(text.contains("/share"));
}

#[test]
fn test_parse_system_no_args() {
    let (cmd, args) = parse_command("/system").expect("failed to parse /system");
    assert_eq!(cmd, "system");
    assert_eq!(args, "");
}

#[test]
fn test_parse_system_show() {
    let (cmd, args) = parse_command("/system show").expect("failed to parse /system show");
    assert_eq!(cmd, "system");
    assert_eq!(args, "show");
}

#[test]
fn test_parse_system_set() {
    let (cmd, args) = parse_command("/system set You are a helpful assistant.").expect("failed to parse /system set");
    assert_eq!(cmd, "system");
    assert_eq!(args, "set You are a helpful assistant.");
}

#[test]
fn test_parse_system_append() {
    let (cmd, args) = parse_command("/system append Always be concise.").expect("failed to parse /system append");
    assert_eq!(cmd, "system");
    assert_eq!(args, "append Always be concise.");
}

#[test]
fn test_parse_system_reset() {
    let (cmd, args) = parse_command("/system reset").expect("failed to parse /system reset");
    assert_eq!(cmd, "system");
    assert_eq!(args, "reset");
}

#[test]
fn test_parse_system_file() {
    let (cmd, args) = parse_command("/system file /tmp/prompt.md").expect("failed to parse /system file");
    assert_eq!(cmd, "system");
    assert_eq!(args, "file /tmp/prompt.md");
}

#[test]
fn test_completions_system() {
    let results = completions("/sy");
    assert!(results.iter().any(|c| c.display == "system"), "results: {:?}", results);
}

#[test]
fn test_help_text_includes_system() {
    let text = help_text();
    assert!(text.contains("/system"));
}

#[test]
fn test_parse_editor() {
    let (cmd, args) = parse_command("/editor").expect("failed to parse /editor");
    assert_eq!(cmd, "editor");
    assert_eq!(args, "");
}

#[test]
fn test_completions_editor() {
    let results = completions("/ed");
    assert!(results.iter().any(|c| c.display == "editor"), "results: {:?}", results);
}

#[test]
fn test_help_text_includes_editor() {
    let text = help_text();
    assert!(text.contains("/editor"));
}

#[test]
fn test_account_subcommands_shown() {
    let results = completions("/account ");
    assert!(!results.is_empty(), "should show subcommands for /account");
    assert!(results.iter().any(|c| c.display.starts_with("switch")));
    assert!(results.iter().any(|c| c.display.starts_with("login")));
}

#[test]
fn test_account_subcommand_filter() {
    let results = completions("/account sw");
    assert_eq!(results.len(), 1);
    assert!(results[0].display.starts_with("switch"));
}

#[test]
fn test_account_subcommand_after_typing_args_hides() {
    let results = completions("/account switch foo");
    assert!(results.is_empty(), "should hide menu after typing args");
}

#[test]
fn test_think_subcommands() {
    let results = completions("/think ");
    assert!(results.iter().any(|c| c.display == "off"));
    assert!(results.iter().any(|c| c.display == "max"));
}

#[test]
fn test_no_subcommands_for_clear() {
    let results = completions("/clear ");
    assert!(results.is_empty());
}

#[test]
fn test_parse_fork() {
    let (cmd, args) = parse_command("/fork").expect("failed to parse /fork");
    assert_eq!(cmd, "fork");
    assert_eq!(args, "");
}

#[test]
fn test_parse_fork_with_args() {
    let (cmd, args) = parse_command("/fork try different approach").expect("failed to parse /fork with args");
    assert_eq!(cmd, "fork");
    assert_eq!(args, "try different approach");
}

#[test]
fn test_parse_rewind() {
    let (cmd, args) = parse_command("/rewind 5").expect("failed to parse /rewind");
    assert_eq!(cmd, "rewind");
    assert_eq!(args, "5");
}

#[test]
fn test_parse_branches() {
    let (cmd, args) = parse_command("/branches").expect("failed to parse /branches");
    assert_eq!(cmd, "branches");
    assert_eq!(args, "");
}

#[test]
fn test_parse_switch() {
    let (cmd, args) = parse_command("/switch main").expect("failed to parse /switch");
    assert_eq!(cmd, "switch");
    assert_eq!(args, "main");
}

#[test]
fn test_parse_label() {
    let (cmd, args) = parse_command("/label checkpoint").expect("failed to parse /label");
    assert_eq!(cmd, "label");
    assert_eq!(args, "checkpoint");
}

#[test]
fn test_completions_fork() {
    let results = completions("/fo");
    assert!(results.iter().any(|c| c.display == "fork"), "results: {:?}", results);
}

#[test]
fn test_completions_branches() {
    let results = completions("/br");
    assert!(results.iter().any(|c| c.display == "branches"), "results: {:?}", results);
}

#[test]
fn test_help_text_includes_branch_commands() {
    let text = help_text();
    assert!(text.contains("/fork"));
    assert!(text.contains("/rewind"));
    assert!(text.contains("/branches"));
    assert!(text.contains("/switch"));
    assert!(text.contains("/label"));
}

// Registry tests
#[test]
fn test_simple_registry_check() {
    // Very simple test to verify registry basics
    let builtin = BuiltinSlashContributor;
    let cmds = builtin.slash_commands();
    assert!(!cmds.is_empty());
}

#[test]
fn test_registry_build_from_builtins() {
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, conflicts) = SlashRegistry::build(&contributors);

    // Should have no conflicts when building from a single contributor
    assert_eq!(conflicts.len(), 0);

    // Should have all builtin commands
    assert_eq!(registry.all_commands().len(), 42);

    // Verify a few specific commands are present
    assert!(registry.get("help").is_some());
    assert!(registry.get("model").is_some());
    assert!(registry.get("fork").is_some());
    assert!(registry.get("system").is_some());
}

#[test]
fn test_registry_conflict_resolution() {
    use crate::registry::PRIORITY_PLUGIN;

    // Create a mock contributor with a conflicting command
    struct MockContributor;
    impl SlashContributor for MockContributor {
        fn slash_commands(&self) -> Vec<SlashCommandDef> {
            vec![SlashCommandDef {
                name: "help".to_string(),
                description: "Plugin help override".to_string(),
                help: "Overridden help".to_string(),
                accepts_args: false,
                subcommands: vec![],
                handler: Box::new(handlers::info::HelpHandler),
                priority: PRIORITY_PLUGIN, // Higher than builtin
                source: "test_plugin".to_string(),
                leader_key: None,
            }]
        }
    }

    let builtin = BuiltinSlashContributor;
    let mock = MockContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin, &mock];
    let (registry, conflicts) = SlashRegistry::build(&contributors);

    // Should have one conflict (help)
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].key, "help");
    assert_eq!(conflicts[0].winner, "test_plugin");
    assert_eq!(conflicts[0].loser, "builtin");

    // The plugin version should win
    let help_cmd = registry.get("help").expect("help command should be registered");
    assert_eq!(help_cmd.description, "Plugin help override");
    assert_eq!(help_cmd.source, "test_plugin");
}

#[test]
fn test_registry_completions() {
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, _) = SlashRegistry::build(&contributors);

    // Test prefix matching
    let completions = registry.completions("he");
    assert!(completions.iter().any(|c| c.name == "help"));

    // Test empty partial returns all
    let all_completions = registry.completions("");
    assert_eq!(all_completions.len(), 42);

    // Test no matches
    let no_match = registry.completions("xyz");
    assert_eq!(no_match.len(), 0);
}

#[test]
fn test_registry_completions_from_registry_function() {
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, _) = SlashRegistry::build(&contributors);

    // Test the completions_from_registry function
    let results = completions_from_registry(&registry, "/he");
    assert!(results.iter().any(|c| c.display == "help"));

    // Test with subcommands
    let results = completions_from_registry(&registry, "/account ");
    assert!(!results.is_empty());
    assert!(results.iter().any(|c| c.display.starts_with("switch")));
}

#[test]
fn test_registry_dispatch_unknown_falls_through() {
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, _) = SlashRegistry::build(&contributors);

    // Create a minimal SlashContext for testing
    // We'll use a channel that we can check for messages
    let (cmd_tx, _cmd_rx) = tokio::sync::mpsc::unbounded_channel();
    let (panel_tx, _panel_rx) = tokio::sync::mpsc::unbounded_channel();

    let model = "test-model".to_string();
    let cwd = std::env::current_dir().expect("failed to get current dir").to_string_lossy().to_string();
    let theme = crate::tui::theme::Theme::dark();
    let mut app = crate::tui::app::App::new(model, cwd, theme);

    let mut ctx = handlers::SlashContext {
        app: &mut app,
        cmd_tx: &cmd_tx,
        plugin_manager: None,
        panel_tx: &panel_tx,
        db: &None,
        session_manager: &mut None,
    };

    // Dispatch an unknown command (should fall through to prompt template handler)
    registry.dispatch("unknown_command", "test args", &mut ctx);

    // The test passes if no panic occurred (prompt template handler doesn't fail)
}

#[test]
fn test_registry_help_text_via_all_commands() {
    let builtin = BuiltinSlashContributor;
    let contributors: Vec<&dyn SlashContributor> = vec![&builtin];
    let (registry, _) = SlashRegistry::build(&contributors);

    let all_cmds = registry.all_commands();
    assert_eq!(all_cmds.len(), 42);

    // Commands should be sorted
    let names: Vec<_> = all_cmds.iter().map(|c| &c.name).collect();
    let mut sorted_names = names.clone();
    sorted_names.sort();
    assert_eq!(names, sorted_names);

    // Verify all expected commands are present
    assert!(all_cmds.iter().any(|c| c.name == "help"));
    assert!(all_cmds.iter().any(|c| c.name == "clear"));
    assert!(all_cmds.iter().any(|c| c.name == "fork"));
    assert!(all_cmds.iter().any(|c| c.name == "system"));
}
