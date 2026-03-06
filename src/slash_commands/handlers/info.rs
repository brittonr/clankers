//! Info slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::slash_commands;

pub struct HelpHandler;

impl SlashHandler for HelpHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system(slash_commands::help_text(), false);
    }
}

pub struct StatusHandler;

impl SlashHandler for StatusHandler {
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
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system(format!("clankers {}", env!("CARGO_PKG_VERSION")), false);
    }
}

pub struct QuitHandler;

impl SlashHandler for QuitHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.should_quit = true;
    }
}
