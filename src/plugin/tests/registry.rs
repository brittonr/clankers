use crate::plugin::registry;

// ── Registry tests ───────────────────────────────────────────────

#[test]
fn registry_register_and_list() {
    let mut reg = registry::PluginRegistry::new();
    reg.register_tool("clankers-test-plugin", "test_echo");
    reg.register_tool("clankers-test-plugin", "test_reverse");
    reg.register_command("clankers-test-plugin", "test");

    let tools = reg.all_tools();
    assert_eq!(tools.len(), 2);
    assert!(tools.contains(&("clankers-test-plugin", "test_echo")));
    assert!(tools.contains(&("clankers-test-plugin", "test_reverse")));

    let commands = reg.all_commands();
    assert_eq!(commands.len(), 1);
    assert!(commands.contains(&("clankers-test-plugin", "test")));
}

#[test]
fn registry_multiple_plugins() {
    let mut reg = registry::PluginRegistry::new();
    reg.register_tool("plugin-a", "tool-1");
    reg.register_tool("plugin-b", "tool-2");

    let tools = reg.all_tools();
    assert_eq!(tools.len(), 2);
}
