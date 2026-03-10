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
            use std::fmt::Write;

            let mut out = String::from("Available tools:\n\n");
            let max_name = ctx.app.tool_info.iter().map(|(n, _, _)| n.len()).max().unwrap_or(0);
            let mut current_source = String::new();
            for (name, description, source) in &ctx.app.tool_info {
                if *source != current_source {
                    if !current_source.is_empty() {
                        out.push('\n');
                    }
                    writeln!(out, "  ── {} ──", source).unwrap();
                    current_source.clone_from(source);
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
                writeln!(out, "  {} {:<width$}  {}", status, name, desc, width = max_name).unwrap();
            }
            let enabled = ctx.app.tool_info.iter().filter(|(n, _, _)| !ctx.app.disabled_tools.contains(n)).count();
            write!(out, "\n  {}/{} tool(s) enabled", enabled, ctx.app.tool_info.len()).unwrap();
            ctx.app.push_system(out, false);
        }
    }
}

pub struct PluginHandler;

impl SlashHandler for PluginHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "plugin",
            description: "Manage plugins",
            help: "Manage loaded plugins.\n\n\
                   Usage:\n  \
                   /plugin              — list all plugins\n  \
                   /plugin <name>       — show details for a plugin\n  \
                   /plugin enable <n>   — enable a disabled plugin\n  \
                   /plugin disable <n>  — disable a plugin\n  \
                   /plugin reload [n]   — reload one or all plugins",
            accepts_args: true,
            subcommands: vec![
                ("enable", "Enable a disabled plugin"),
                ("disable", "Disable a plugin"),
                ("reload", "Reload a plugin (or all)"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let Some(pm) = ctx.plugin_manager else {
            ctx.app.push_system("Plugin system not initialized.".to_string(), true);
            return;
        };

        let args = args.trim();

        // /plugin enable <name>
        if let Some(name) = args.strip_prefix("enable").map(|s| s.trim()) {
            if name.is_empty() {
                ctx.app.push_system("Usage: /plugin enable <name>".to_string(), true);
                return;
            }
            let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
            match mgr.enable(name) {
                Ok(()) => {
                    ctx.app.push_system(format!("Plugin '{}' enabled.", name), false);
                    save_disabled_plugins(&mgr);
                }
                Err(e) => ctx.app.push_system(format!("Failed to enable '{}': {}", name, e), true),
            }
            return;
        }

        // /plugin disable <name>
        if let Some(name) = args.strip_prefix("disable").map(|s| s.trim()) {
            if name.is_empty() {
                ctx.app.push_system("Usage: /plugin disable <name>".to_string(), true);
                return;
            }
            let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
            match mgr.disable(name) {
                Ok(()) => {
                    ctx.app.push_system(format!("Plugin '{}' disabled.", name), false);
                    save_disabled_plugins(&mgr);
                }
                Err(e) => ctx.app.push_system(format!("Failed to disable '{}': {}", name, e), true),
            }
            return;
        }

        // /plugin reload [name]
        if let Some(rest) = args.strip_prefix("reload") {
            let name = rest.trim();
            let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
            if name.is_empty() {
                mgr.reload_all();
                ctx.app.push_system("All plugins reloaded.".to_string(), false);
            } else {
                match mgr.reload(name) {
                    Ok(()) => ctx.app.push_system(format!("Plugin '{}' reloaded.", name), false),
                    Err(e) => ctx.app.push_system(format!("Failed to reload '{}': {}", name, e), true),
                }
            }
            return;
        }

        let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());

        if args.is_empty() {
            // List all plugins
            let plugins = mgr.list();
            if plugins.is_empty() {
                ctx.app.push_system("No plugins discovered.".to_string(), false);
            } else {
                use std::fmt::Write;

                let mut out = String::from("Plugins:\n\n");
                for p in plugins {
                    let state = match &p.state {
                        crate::plugin::PluginState::Active => "✓",
                        crate::plugin::PluginState::Loaded => "○",
                        crate::plugin::PluginState::Error(e) => {
                            writeln!(out, "  ✗ {} v{} — Error: {}", p.name, p.version, e).unwrap();
                            continue;
                        }
                        crate::plugin::PluginState::Disabled => "−",
                    };
                    writeln!(
                        out,
                        "  {} {} v{} — {} (tools: {})",
                        state,
                        p.name,
                        p.version,
                        p.manifest.description,
                        if p.manifest.tools.is_empty() {
                            "none".to_string()
                        } else {
                            p.manifest.tools.join(", ")
                        },
                    )
                    .unwrap();
                }
                write!(
                    out,
                    "\n  ✓ active  ○ loaded  − disabled  ✗ error\n  \
                     Use /plugin enable|disable|reload <name>"
                )
                .unwrap();
                ctx.app.push_system(out, false);
            }
        } else {
            // Show specific plugin
            if let Some(p) = mgr.get(args) {
                use std::fmt::Write;

                let mut out = String::new();
                writeln!(out, "Plugin: {} v{}", p.name, p.version).unwrap();
                writeln!(out, "State: {:?}", p.state).unwrap();
                writeln!(out, "Description: {}", p.manifest.description).unwrap();
                writeln!(out, "Path: {}", p.path.display()).unwrap();
                writeln!(
                    out,
                    "Tools: {}",
                    if p.manifest.tools.is_empty() {
                        "none".into()
                    } else {
                        p.manifest.tools.join(", ")
                    }
                )
                .unwrap();
                writeln!(
                    out,
                    "Commands: {}",
                    if p.manifest.commands.is_empty() {
                        "none".into()
                    } else {
                        p.manifest.commands.join(", ")
                    }
                )
                .unwrap();
                writeln!(
                    out,
                    "Events: {}",
                    if p.manifest.events.is_empty() {
                        "none".into()
                    } else {
                        p.manifest.events.join(", ")
                    }
                )
                .unwrap();
                write!(
                    out,
                    "Permissions: {}",
                    if p.manifest.permissions.is_empty() {
                        "none".into()
                    } else {
                        p.manifest.permissions.join(", ")
                    }
                )
                .unwrap();
                ctx.app.push_system(out, false);
            } else {
                ctx.app.push_system(format!("Plugin '{}' not found.", args), true);
            }
        }
    }
}

/// Persist the set of disabled plugins to disk.
fn save_disabled_plugins(mgr: &crate::plugin::PluginManager) {
    let disabled = mgr.disabled_plugins();
    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("clankers");
    std::fs::create_dir_all(&config_dir).ok();
    let path = config_dir.join("disabled-plugins.json");
    if let Ok(json) = serde_json::to_string_pretty(&disabled)
        && let Err(e) = std::fs::write(&path, json)
    {
        tracing::warn!("Failed to save disabled plugins: {}", e);
    }
}

/// Load the set of disabled plugins from disk.
pub fn load_disabled_plugins() -> Vec<String> {
    let config_dir = dirs::config_dir().unwrap_or_else(|| std::path::PathBuf::from(".")).join("clankers");
    let path = config_dir.join("disabled-plugins.json");
    match std::fs::read_to_string(&path) {
        Ok(contents) => serde_json::from_str(&contents).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}
