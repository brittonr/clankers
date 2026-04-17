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
                ctx.cmd_tx.send(crate::modes::interactive::AgentCommand::SetDisabledTools(disabled)).ok();
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
            ctx.cmd_tx.send(crate::modes::interactive::AgentCommand::SetDisabledTools(disabled)).ok();
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
                    writeln!(out, "  ── {} ──", source).ok();
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
                writeln!(out, "  {} {:<width$}  {}", status, name, desc, width = max_name).ok();
            }
            let enabled = ctx.app.tool_info.iter().filter(|(n, _, _)| !ctx.app.disabled_tools.contains(n)).count();
            write!(out, "\n  {}/{} tool(s) enabled", enabled, ctx.app.tool_info.len()).ok();
            write!(out, "\n  Tiers: core + specialty + orchestration (use --tools to change)").ok();
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
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let Some(pm) = ctx.plugin_manager else {
            ctx.app.push_system("Plugin system not initialized.".to_string(), true);
            return;
        };

        let args = args.trim();

        if let Some(name) = args.strip_prefix("enable").map(|s| s.trim()) {
            plugin_toggle(pm, name, true, ctx);
        } else if let Some(name) = args.strip_prefix("disable").map(|s| s.trim()) {
            plugin_toggle(pm, name, false, ctx);
        } else if let Some(rest) = args.strip_prefix("reload") {
            plugin_reload(pm, rest.trim(), ctx);
        } else if args.is_empty() {
            plugin_list(pm, ctx);
        } else {
            plugin_show(pm, args, ctx);
        }
    }
}

type PluginMutex = std::sync::Arc<std::sync::Mutex<crate::plugin::PluginManager>>;

fn plugin_toggle(pm: &PluginMutex, name: &str, enable: bool, ctx: &mut SlashContext<'_>) {
    let verb = if enable { "enable" } else { "disable" };
    if name.is_empty() {
        ctx.app.push_system(format!("Usage: /plugin {} <name>", verb), true);
        return;
    }
    let mut mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
    let result = if enable { mgr.enable(name) } else { mgr.disable(name) };
    match result {
        Ok(()) => {
            ctx.app.push_system(format!("Plugin '{}' {}d.", name, verb), false);
            save_disabled_plugins(&mgr);
        }
        Err(e) => ctx.app.push_system(format!("Failed to {} '{}': {}", verb, name, e), true),
    }
}

fn plugin_reload(pm: &PluginMutex, name: &str, ctx: &mut SlashContext<'_>) {
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
}

fn plugin_list(pm: &PluginMutex, ctx: &mut SlashContext<'_>) {
    use std::fmt::Write;

    let host = crate::plugin::PluginHostFacade::new(std::sync::Arc::clone(pm));
    let plugins = host.summaries();
    if plugins.is_empty() {
        ctx.app.push_system("No plugins discovered.".to_string(), false);
        return;
    }

    let mut out = String::from("Plugins:\n\n");
    for p in plugins {
        let icon = match p.state.as_str() {
            "Active" => "✓",
            "Loaded" | "Starting" => "○",
            "Backoff" => "↺",
            "Disabled" => "−",
            _ => "✗",
        };
        let tools = if p.tools.is_empty() { "none".to_string() } else { p.tools.join(", ") };
        writeln!(out, "  {} {} v{}", icon, p.name, p.version).ok();
        writeln!(out, "      kind: {}  state: {}", p.kind, p.state).ok();
        writeln!(out, "      tools: {}", tools).ok();
        if let Some(error) = p.last_error {
            writeln!(out, "      last error: {}", error).ok();
        }
    }
    write!(out, "\n  ✓ active  ○ loaded/starting  ↺ backoff  − disabled  ✗ error\n  Use /plugin enable|disable|reload <name>").ok();
    ctx.app.push_system(out, false);
}

fn plugin_show(pm: &PluginMutex, name: &str, ctx: &mut SlashContext<'_>) {
    use std::fmt::Write;

    let mgr = pm.lock().unwrap_or_else(|e| e.into_inner());
    let Some(p) = mgr.get(name) else {
        ctx.app.push_system(format!("Plugin '{}' not found.", name), true);
        return;
    };

    let join_or_none = |v: &[String]| -> String { if v.is_empty() { "none".into() } else { v.join(", ") } };
    let tools = p.declared_tool_inventory();

    let mut out = String::new();
    writeln!(out, "Plugin: {} v{}", p.name, p.version).ok();
    writeln!(out, "Kind: {}", p.manifest.kind).ok();
    writeln!(out, "State: {}", p.state.summary_label()).ok();
    if let Some(error) = p.state.last_error() {
        writeln!(out, "Last error: {}", error).ok();
    }
    writeln!(out, "Description: {}", p.manifest.description).ok();
    writeln!(out, "Path: {}", p.path.display()).ok();
    writeln!(out, "Tools: {}", join_or_none(&tools)).ok();
    writeln!(out, "Commands: {}", join_or_none(&p.manifest.commands)).ok();
    writeln!(out, "Events: {}", join_or_none(&p.manifest.events)).ok();
    write!(out, "Permissions: {}", join_or_none(&p.manifest.permissions)).ok();
    ctx.app.push_system(out, false);
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
