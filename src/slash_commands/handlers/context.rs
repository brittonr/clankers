//! Context and navigation slash command handlers.

use clankers_tui_types::BlockEntry;

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;

pub struct ClearHandler;

impl SlashHandler for ClearHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "clear",
            description: "Clear conversation history",
            help: "Clears the visible message history. Does not affect the agent's context window.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }

    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.conversation.blocks.clear();
        let _ = ctx.cmd_tx.send(AgentCommand::ClearHistory);
        ctx.app.push_system("Conversation cleared.".to_string(), false);
        ctx.app.conversation.scroll.scroll_to_top();
    }
}

pub struct ResetHandler;

impl SlashHandler for ResetHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "reset",
            description: "Reset conversation and context",
            help: "Clears conversation history and resets the agent context, starting fresh.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }

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
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "compact",
            description: "Summarize conversation to save tokens",
            help: "Asks the model to create a compact summary of the conversation so far, \
                   replacing the full history to reduce token usage.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: Some(super::super::LeaderBinding {
                key: 'C',
                placement: crate::tui::components::leader_menu::MenuPlacement::Root,
                label: Some("compact"),
            }),
        }
    }

    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        ctx.app.push_system("Compact mode is not yet implemented.".to_string(), false);
    }
}

pub struct UndoHandler;

impl SlashHandler for UndoHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "undo",
            description: "Remove last conversation turn",
            help: "Removes the last user message and assistant response from the conversation.",
            accepts_args: false,
            subcommands: vec![],
            leader_key: None,
        }
    }

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

// ── Navigation handlers (merged from navigation.rs) ─────────────────────────

pub struct CdHandler;

impl SlashHandler for CdHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "cd",
            description: "Change working directory",
            help: "Change the working directory. Usage: /cd <path>",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system(format!("Current directory: {}\n\nUsage: /cd <path>", ctx.app.cwd), false);
        } else {
            let new_path = if args.starts_with('/') {
                std::path::PathBuf::from(args)
            } else {
                std::path::PathBuf::from(&ctx.app.cwd).join(args)
            };
            match new_path.canonicalize() {
                Ok(canonical) if canonical.is_dir() => {
                    ctx.app.cwd = canonical.to_string_lossy().to_string();
                    ctx.app.git_status.set_cwd(&ctx.app.cwd);
                    ctx.app.push_system(format!("Changed directory to: {}", ctx.app.cwd), false);
                }
                Ok(_) => {
                    ctx.app.push_system(format!("Not a directory: {}", args), true);
                }
                Err(e) => {
                    ctx.app.push_system(format!("Invalid path '{}': {}", args, e), true);
                }
            }
        }
    }
}

pub struct ShellHandler;

impl SlashHandler for ShellHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "shell",
            description: "Run a shell command directly",
            help: "Execute a shell command without going through the agent. Usage: /shell <command>",
            accepts_args: true,
            subcommands: vec![],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /shell <command>".to_string(), false);
        } else {
            match std::process::Command::new("sh").arg("-c").arg(args).current_dir(&ctx.app.cwd).output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    let mut result = String::new();
                    if !stdout.is_empty() {
                        result.push_str(&stdout);
                    }
                    if !stderr.is_empty() {
                        if !result.is_empty() {
                            result.push('\n');
                        }
                        result.push_str(&stderr);
                    }
                    if result.is_empty() {
                        result = format!("(exit code: {})", output.status.code().unwrap_or(-1));
                    }
                    ctx.app.push_system(result, !output.status.success());
                }
                Err(e) => {
                    ctx.app.push_system(format!("Failed to run command: {}", e), true);
                }
            }
        }
    }
}
