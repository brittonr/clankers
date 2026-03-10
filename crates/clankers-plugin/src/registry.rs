//! Plugin tool/command registry

use std::collections::HashMap;

/// Registry of tools and commands provided by plugins
#[derive(Debug, Default)]
pub struct PluginRegistry {
    /// plugin_name -> list of tool names
    pub tools: HashMap<String, Vec<String>>,
    /// plugin_name -> list of command names
    pub commands: HashMap<String, Vec<String>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register_tool(&mut self, plugin: &str, tool: &str) {
        self.tools.entry(plugin.to_string()).or_default().push(tool.to_string());
    }

    pub fn register_command(&mut self, plugin: &str, command: &str) {
        self.commands.entry(plugin.to_string()).or_default().push(command.to_string());
    }

    pub fn all_tools(&self) -> Vec<(&str, &str)> {
        self.tools
            .iter()
            .flat_map(|(plugin, tools)| tools.iter().map(move |t| (plugin.as_str(), t.as_str())))
            .collect()
    }

    pub fn all_commands(&self) -> Vec<(&str, &str)> {
        self.commands
            .iter()
            .flat_map(|(plugin, cmds)| cmds.iter().map(move |c| (plugin.as_str(), c.as_str())))
            .collect()
    }
}
