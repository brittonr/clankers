//! Context slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;
use crate::tui::components::block::BlockEntry;

pub struct ClearHandler;

impl SlashHandler for ClearHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.conversation.blocks.clear();
        let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
        ctx.app.push_system("Conversation cleared.".to_string(), false);
        ctx.app.conversation.scroll.scroll_to_top();
    }
}

pub struct ResetHandler;

impl SlashHandler for ResetHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.conversation.blocks.clear();
        ctx.app.conversation.all_blocks.clear();
        ctx.app.conversation.active_block = None;
        ctx.app.streaming.text.clear();
        ctx.app.streaming.thinking.clear();
        ctx.app.total_tokens = 0;
        ctx.app.total_cost = 0.0;
        ctx.app.conversation.focused_block = None;
        let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
        let _ = ctx.cmd_tx.send(AgentCommand::ResetCancel);
        ctx.app.push_system("Session reset. Context and history cleared.".to_string(), false);
        ctx.app.conversation.scroll.scroll_to_top();
    }
}

pub struct CompactHandler;

impl SlashHandler for CompactHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system("Compact mode is not yet implemented.".to_string(), false);
    }
}

pub struct UndoHandler;

impl SlashHandler for UndoHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        let mut removed = false;
        for i in (0..ctx.app.conversation.blocks.len()).rev() {
            if matches!(ctx.app.conversation.blocks[i], BlockEntry::Conversation(_)) {
                ctx.app.conversation.blocks.remove(i);
                removed = true;
                break;
            }
        }
        if removed {
            ctx.app.push_system("Last conversation block removed.".to_string(), false);
        } else {
            ctx.app.push_system("Nothing to undo.".to_string(), false);
        }
    }
}
