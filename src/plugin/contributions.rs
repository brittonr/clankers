//! Plugin contributions to the leader menu, slash commands, and filesystem discovery.
//!
//! Uses wrapper types to satisfy the orphan rule — `PluginManager` lives in
//! `clankers-plugin` while the traits live in `clankers-tui` and main crate.

use std::collections::HashMap;
use std::path::Path;

use clankers_plugin::PluginInfo;
use clankers_plugin::PluginManager;

// ---------------------------------------------------------------------------
// MenuContributor — wrapper for PluginManager → leader menu entries
// ---------------------------------------------------------------------------

/// Wrapper that implements `MenuContributor` for a borrowed `PluginManager`.
pub struct PluginMenuContributor<'a>(pub &'a PluginManager);

impl crate::tui::components::leader_menu::MenuContributor for PluginMenuContributor<'_> {
    fn menu_items(&self) -> Vec<crate::tui::components::leader_menu::MenuContribution> {
        use clankers_tui_types::LeaderAction;
        use clankers_tui_types::MenuContribution;
        use clankers_tui_types::MenuPlacement;
        use clankers_tui_types::PRIORITY_PLUGIN;

        self.0
            .active_plugin_infos()
            .flat_map(|plugin| {
                plugin.manifest.leader_menu.iter().filter_map(move |entry| {
                    // Validate: key must be printable ASCII, command must start with /
                    if !entry.key.is_ascii_graphic() {
                        tracing::warn!(
                            plugin = %plugin.name,
                            key = %entry.key,
                            "plugin leader_menu entry has non-printable key, skipping"
                        );
                        return None;
                    }
                    if !entry.command.starts_with('/') {
                        tracing::warn!(
                            plugin = %plugin.name,
                            command = %entry.command,
                            "plugin leader_menu command must start with '/', skipping"
                        );
                        return None;
                    }
                    if entry.label.is_empty() {
                        tracing::warn!(
                            plugin = %plugin.name,
                            "plugin leader_menu entry has empty label, skipping"
                        );
                        return None;
                    }
                    Some(MenuContribution {
                        key: entry.key,
                        label: entry.label.clone(),
                        action: LeaderAction::SlashCommand(entry.command.clone()),
                        placement: match &entry.submenu {
                            Some(name) => MenuPlacement::Submenu(name.clone()),
                            None => MenuPlacement::Root,
                        },
                        priority: PRIORITY_PLUGIN,
                        source: plugin.name.clone(),
                    })
                })
            })
            .collect()
    }
}

// ---------------------------------------------------------------------------
// SlashContributor — wrapper for PluginManager → slash command registration
// ---------------------------------------------------------------------------

/// Wrapper that implements `SlashContributor` for a borrowed `PluginManager`.
pub struct PluginSlashContributor<'a>(pub &'a PluginManager);

impl crate::slash_commands::SlashContributor for PluginSlashContributor<'_> {
    fn slash_commands(&self) -> Vec<crate::slash_commands::SlashCommandDef> {
        use clankers_tui_types::PRIORITY_PLUGIN;

        self.0
            .active_plugin_infos()
            .flat_map(|plugin| {
                plugin.manifest.commands.iter().map(move |command_name| {
                    let plugin_name = plugin.name.clone();
                    let cmd_name = command_name.clone();

                    crate::slash_commands::SlashCommandDef {
                        name: cmd_name.clone(),
                        description: format!("Plugin command: {}", cmd_name),
                        help: format!(
                            "Execute the '{}' command from the '{}' plugin",
                            cmd_name, plugin_name
                        ),
                        accepts_args: true,
                        subcommands: vec![],
                        handler: Box::new(PluginSlashHandler {
                            plugin_name: plugin_name.clone(),
                            command_name: cmd_name,
                        }),
                        priority: PRIORITY_PLUGIN,
                        source: format!("plugin:{}", plugin_name),
                    }
                })
            })
            .collect()
    }
}

/// Handler for plugin slash commands
struct PluginSlashHandler {
    plugin_name: String,
    command_name: String,
}

impl crate::slash_commands::handlers::SlashHandler for PluginSlashHandler {
    fn command(&self) -> crate::slash_commands::SlashCommand {
        // PluginSlashHandler is dynamic — command metadata comes from the plugin manifest.
        // We return a placeholder. The real metadata is provided by SlashContributor above.
        crate::slash_commands::SlashCommand {
            name: Box::leak(self.command_name.clone().into_boxed_str()),
            description: Box::leak(
                format!("Plugin command: {}", self.command_name).into_boxed_str(),
            ),
            help: Box::leak(
                format!(
                    "Execute the '{}' command from the '{}' plugin",
                    self.command_name, self.plugin_name
                )
                .into_boxed_str(),
            ),
            accepts_args: true,
            subcommands: vec![],
        }
    }

    fn handle(&self, args: &str, ctx: &mut crate::slash_commands::handlers::SlashContext<'_>) {
        // Try to call the plugin's handle_command export via the plugin bridge
        if let Some(pm_arc) = ctx.plugin_manager {
            if let Ok(pm) = pm_arc.lock() {
                // Construct the input JSON for the plugin
                let input = serde_json::json!({
                    "command": self.command_name,
                    "args": args,
                });

                let input_str = match serde_json::to_string(&input) {
                    Ok(s) => s,
                    Err(e) => {
                        ctx.app
                            .push_system(format!("Failed to serialize command: {}", e), true);
                        return;
                    }
                };

                // Call the plugin's handle_command function
                match pm.call_plugin(&self.plugin_name, "handle_command", &input_str) {
                    Ok(response) => {
                        // Parse the response and show it to the user
                        match serde_json::from_str::<serde_json::Value>(&response) {
                            Ok(json) => {
                                // If there's a "message" field, show it
                                if let Some(message) =
                                    json.get("message").and_then(|v| v.as_str())
                                {
                                    ctx.app.push_system(message.to_string(), false);
                                } else {
                                    // Otherwise, show the raw JSON response
                                    ctx.app.push_system(response, false);
                                }
                            }
                            Err(_) => {
                                // Not JSON, just show the raw response
                                ctx.app.push_system(response, false);
                            }
                        }
                    }
                    Err(e) => {
                        ctx.app.push_system(format!("Plugin error: {}", e), true);
                    }
                }
            } else {
                ctx.app
                    .push_system("Failed to acquire plugin manager lock".to_string(), true);
            }
        } else {
            ctx.app
                .push_system("Plugin manager not available".to_string(), true);
        }
    }
}

/// Load plugins from a directory into the provided HashMap.
pub fn load_plugins_from_dir(dir: &Path, plugins: &mut HashMap<String, PluginInfo>) {
    clankers_plugin::load_plugins_from_dir(dir, plugins);
}
