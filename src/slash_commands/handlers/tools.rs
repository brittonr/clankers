//! Tools slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct ToolsHandler;

impl SlashHandler for ToolsHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "tools",
            description: "List or toggle available tools",
            help: "Lists all tools available to the agent.\n\nUsage:\n  /tools           — list all tools\n  /tools toggle     — open tool toggle menu\n  /tools enable X   — enable tool X\n  /tools disable X  — disable tool X",
            accepts_args: true,
            subcommands: vec![
                ("toggle", "Open tool toggle menu"),
                ("enable", "Enable a tool by name"),
                ("disable", "Disable a tool by name"),
            ],
            leader_key: Some(super::super::LeaderBinding {
                key: 'w',
                placement: clankers_tui_types::MenuPlacement::Root,
                label: Some("tools"),
            }),
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let args = args.trim();

        if args == "toggle" {
            let tools = ctx.app.tool_info.clone();
            ctx.app.overlays.tool_toggle.open(tools, &ctx.app.disabled_tools);
            return;
        }

        if let Some(name) = args.strip_prefix("enable").map(|s| s.trim()) {
            if name.is_empty() {
                ctx.app.push_system("Usage: /tools enable <name>".to_string(), true);
                return;
            }
            if ctx.app.disabled_tools.remove(name) {
                let disabled = ctx.app.disabled_tools.clone();
                let _ = ctx.cmd_tx.send(crate::modes::interactive::AgentCommand::SetDisabledTools(disabled));
                ctx.app.push_system(format!("Tool '{}' enabled.", name), false);
            } else {
                ctx.app.push_system(format!("Tool '{}' is already enabled.", name), false);
            }
            return;
        }

        if let Some(name) = args.strip_prefix("disable").map(|s| s.trim()) {
            if name.is_empty() {
                ctx.app.push_system("Usage: /tools disable <name>".to_string(), true);
                return;
            }
            // Verify the tool exists
            if !ctx.app.tool_info.iter().any(|(n, _, _)| n == name) {
                ctx.app.push_system(format!("Unknown tool '{}'.", name), true);
                return;
            }
            ctx.app.disabled_tools.insert(name.to_string());
            let disabled = ctx.app.disabled_tools.clone();
            let _ = ctx.cmd_tx.send(crate::modes::interactive::AgentCommand::SetDisabledTools(disabled));
            ctx.app.push_system(format!("Tool '{}' disabled.", name), false);
            return;
        }

        // Default: list tools
        if ctx.app.tool_info.is_empty() {
            ctx.app.push_system("No tools available.".to_string(), false);
        } else {
            let mut out = String::from("Available tools:\n\n");
            let max_name = ctx.app.tool_info.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
            let mut current_source = String::new();
            for (name, description, source) in &ctx.app.tool_info {
                if *source != current_source {
                    if !current_source.is_empty() {
                        out.push('\n');
                    }
                    out.push_str(&format!("  ── {} ──\n", source));
                    current_source = source.clone();
                }
                let status = if ctx.app.disabled_tools.contains(name) {
                    "✗"
                } else {
                    "✓"
                };
                let desc = if description.len() > 55 {
                    format!("{}…", &description[..54])
                } else {
                    description.clone()
                };
                out.push_str(&format!("  {} {:<width$}  {}\n", status, name, desc, width = max_name));
            }
            let enabled = ctx.app.tool_info.iter().filter(|(n, _, _)| !ctx.app.disabled_tools.contains(n)).count();
            out.push_str(&format!("\n  {}/{} tool(s) enabled", enabled, ctx.app.tool_info.len()));
            ctx.app.push_system(out, false);
        }
    }
}

pub struct PluginHandler;

impl SlashHandler for PluginHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "plugin",
            description: "Show loaded plugins",
            help: "Lists all discovered and loaded plugins with their status.\n\nUsage: /plugin [name]  — show details for a specific plugin",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if let Some(pm) = ctx.plugin_manager {
            let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
            if args.is_empty() {
                // List all plugins
                let plugins = mgr.list();
                if plugins.is_empty() {
                    ctx.app.push_system(
                        "No plugins loaded.\n\nInstall plugins with: clankers plugin install <path>".to_string(),
                        false,
                    );
                } else {
                    let mut out = String::from("Loaded plugins:\n\n");
                    for p in plugins {
                        let state = match &p.state {
                            crate::plugin::PluginState::Active => "✓",
                            crate::plugin::PluginState::Loaded => "○",
                            crate::plugin::PluginState::Error(e) => {
                                out.push_str(&format!("  ✗ {} v{} — Error: {}\n", p.name, p.version, e));
                                continue;
                            }
                            crate::plugin::PluginState::Disabled => "−",
                        };
                        out.push_str(&format!(
                            "  {} {} v{} — {} (tools: {})\n",
                            state,
                            p.name,
                            p.version,
                            p.manifest.description,
                            if p.manifest.tools.is_empty() {
                                "none".to_string()
                            } else {
                                p.manifest.tools.join(", ")
                            },
                        ));
                    }
                    ctx.app.push_system(out, false);
                }
            } else {
                // Show specific plugin
                if let Some(p) = mgr.get(args.trim()) {
                    let out = format!(
                        "Plugin: {} v{}\nState: {:?}\nDescription: {}\nPath: {}\nTools: {}\nCommands: {}\nEvents: {}\nPermissions: {}",
                        p.name,
                        p.version,
                        p.state,
                        p.manifest.description,
                        p.path.display(),
                        if p.manifest.tools.is_empty() {
                            "none".to_string()
                        } else {
                            p.manifest.tools.join(", ")
                        },
                        if p.manifest.commands.is_empty() {
                            "none".to_string()
                        } else {
                            p.manifest.commands.join(", ")
                        },
                        if p.manifest.events.is_empty() {
                            "none".to_string()
                        } else {
                            p.manifest.events.join(", ")
                        },
                        if p.manifest.permissions.is_empty() {
                            "none".to_string()
                        } else {
                            p.manifest.permissions.join(", ")
                        },
                    );
                    ctx.app.push_system(out, false);
                } else {
                    ctx.app.push_system(format!("Plugin '{}' not found.", args.trim()), true);
                }
            }
        } else {
            ctx.app.push_system("Plugin system not initialized.".to_string(), true);
        }
    }
}
