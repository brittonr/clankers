//! Info slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::slash_commands;

pub struct HelpHandler;

impl SlashHandler for HelpHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "help",
            description: "Show available commands",
            help: "Lists all available slash commands with descriptions.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: Some(super::super::LeaderBinding {
                key: '?',
                placement: crate::tui::components::leader_menu::MenuPlacement::Root,
                label: Some("help"),
            }),
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system(slash_commands::help_text(), false);
    }
}

pub struct StatusHandler;

impl SlashHandler for StatusHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "status",
            description: "Show current settings",
            help: "Displays the current model, token usage, and session information.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        let status = format!(
            "Model: {}\nTokens used: {}\nCost: ${:.4}\nSession: {}\nCWD: {}",
            ctx.app.model, ctx.app.total_tokens, ctx.app.total_cost, ctx.app.session_id, ctx.app.cwd,
        );
        ctx.app.push_system(status, false);
    }
}

pub struct UsageHandler;

impl SlashHandler for UsageHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "usage",
            description: "Show token usage statistics",
            help: "Shows detailed token usage and estimated cost for this session.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        let usage = format!(
            "Token usage:\n  Total tokens: {}\n  Estimated cost: ${:.4}",
            ctx.app.total_tokens, ctx.app.total_cost,
        );
        ctx.app.push_system(usage, false);
    }
}

pub struct VersionHandler;

impl SlashHandler for VersionHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "version",
            description: "Show version information",
            help: "Displays the clankers version and build information.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system(format!("clankers {}", env!("CARGO_PKG_VERSION")), false);
    }
}

pub struct QuitHandler;

impl SlashHandler for QuitHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "quit",
            description: "Quit clankers",
            help: "Exit the application.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.should_quit = true;
    }
}

pub struct LeaderHandler;

impl SlashHandler for LeaderHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "leader",
            description: "Dump leader menu structure (debug)",
            help: "Show the current leader menu structure, including all items,\n\
                   submenus, and their sources. Useful for debugging menu\n\
                   contributions from builtins, plugins, and user config.\n\n\
                   The leader menu (Space in normal mode) is built dynamically from:\n  \
                   1. Built-in keymap actions and slash commands\n  \
                   2. Plugin manifest `leader_menu` entries\n  \
                   3. User config `[leader_menu]` in settings.json\n\n\
                   User config (priority 200) overrides plugins (100), which\n  \
                   override builtins (0). Use `leader_menu.hide` in settings\n  \
                   to remove entries.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }
    
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        let menu = &ctx.app.overlays.leader_menu;
        let root = menu.root_def();
        let submenus = menu.submenu_defs();
        let mut out = String::from("Leader menu structure:\n");

        // Root items
        out.push_str("\n  ── Root ──\n");
        for item in &root.items {
            let action_str = format_leader_action(&item.action);
            out.push_str(&format!("  {:>3}  {:<24} {}\n", item.key, item.label, action_str));
        }

        // Submenus
        for sub in submenus {
            out.push_str(&format!("\n  ── {} ──\n", sub.label));
            for item in &sub.items {
                let action_str = format_leader_action(&item.action);
                out.push_str(&format!("  {:>3}  {:<24} {}\n", item.key, item.label, action_str));
            }
        }

        out.push_str(&format!(
            "\n  {} root items, {} submenus",
            root.items.len(),
            submenus.len(),
        ));

        ctx.app.push_system(out, false);
    }
}

fn format_leader_action(action: &crate::tui::components::leader_menu::LeaderAction) -> String {
    use crate::tui::components::leader_menu::LeaderAction;
    match action {
        LeaderAction::KeymapAction(a) => format!("→ {:?}", a),
        LeaderAction::SlashCommand(cmd) => format!("→ {}", cmd),
        LeaderAction::Submenu(name) => format!("→ [{}…]", name),
    }
}
