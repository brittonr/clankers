//! Memory slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;

pub struct SystemPromptHandler;

impl SystemPromptHandler {
    /// Get the current system prompt from the agent.
    fn get_current_prompt(ctx: &SlashContext<'_>) -> String {
        let (tx, mut rx) = tokio::sync::oneshot::channel();
        let _ = ctx.cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
        rx.try_recv().unwrap_or_else(|_| ctx.app.original_system_prompt.clone())
    }

    fn handle_show(ctx: &mut SlashContext<'_>, full: bool) {
        let prompt = Self::get_current_prompt(ctx);
        let display = if full {
            format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), prompt)
        } else {
            let truncated = if prompt.len() > 500 {
                format!("{}…\n\n*(truncated — use `/system show` for full prompt)*", &prompt[..500])
            } else {
                prompt.clone()
            };
            format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), truncated)
        };
        ctx.app.push_system(display, false);
    }

    fn handle_set(ctx: &mut SlashContext<'_>, text: &str) {
        if text.is_empty() {
            ctx.app.push_system("Usage: /system set <new system prompt text>".to_string(), true);
        } else {
            let new_prompt = text.to_string();
            let len = new_prompt.len();
            let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
            ctx.app.push_system(
                format!("System prompt replaced ({} chars). Takes effect on next message.", len),
                false,
            );
        }
    }

    fn handle_append(ctx: &mut SlashContext<'_>, text: &str) {
        if text.is_empty() {
            ctx.app.push_system("Usage: /system append <text to append>".to_string(), true);
        } else {
            let current = Self::get_current_prompt(ctx);
            let new_prompt = format!("{}\n\n{}", current, text);
            let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
            ctx.app.push_system(
                format!(
                    "Appended {} chars to system prompt. Takes effect on next message.",
                    text.len()
                ),
                false,
            );
        }
    }

    fn handle_prepend(ctx: &mut SlashContext<'_>, text: &str) {
        if text.is_empty() {
            ctx.app.push_system("Usage: /system prepend <text to prepend>".to_string(), true);
        } else {
            let current = Self::get_current_prompt(ctx);
            let new_prompt = format!("{}\n\n{}", text, current);
            let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
            ctx.app.push_system(
                format!(
                    "Prepended {} chars to system prompt. Takes effect on next message.",
                    text.len()
                ),
                false,
            );
        }
    }

    fn handle_reset(ctx: &mut SlashContext<'_>) {
        let original = ctx.app.original_system_prompt.clone();
        let len = original.len();
        let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(original));
        ctx.app.push_system(
            format!("System prompt reset to original ({} chars). Takes effect on next message.", len),
            false,
        );
    }

    fn handle_file(ctx: &mut SlashContext<'_>, path_str: &str) {
        if path_str.is_empty() {
            ctx.app.push_system("Usage: /system file <path>".to_string(), true);
        } else {
            let path = if path_str.starts_with('/') {
                std::path::PathBuf::from(path_str)
            } else {
                std::path::PathBuf::from(&ctx.app.cwd).join(path_str)
            };
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    let len = content.len();
                    let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(content));
                    ctx.app.push_system(
                        format!(
                            "System prompt loaded from {} ({} chars). Takes effect on next message.",
                            path.display(),
                            len
                        ),
                        false,
                    );
                }
                Err(e) => {
                    ctx.app.push_system(format!("Failed to read '{}': {}", path.display(), e), true);
                }
            }
        }
    }
}

impl SlashHandler for SystemPromptHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "system",
            description: "View or modify the system prompt",
            help: "View, replace, append to, or reset the system prompt.\n\n\
                   Usage:\n  \
                   /system              — show the current system prompt (truncated)\n  \
                   /system show         — show the full system prompt\n  \
                   /system set <text>   — replace the system prompt entirely\n  \
                   /system append <text>— append text to the system prompt\n  \
                   /system prepend <text>— prepend text to the system prompt\n  \
                   /system reset        — restore the original system prompt\n  \
                   /system file <path>  — load system prompt from a file",
            accepts_args: true,
            subcommands: vec![
                ("show", "show the full system prompt"),
                ("set <text>", "replace the system prompt"),
                ("append <text>", "append to the system prompt"),
                ("prepend <text>", "prepend to the system prompt"),
                ("reset", "restore the original system prompt"),
                ("file <path>", "load system prompt from a file"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() {
            Self::handle_show(ctx, false);
            return;
        }

        let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
        let subcmd = parts[0].trim();
        let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match subcmd {
            "show" => Self::handle_show(ctx, true),
            "set" => Self::handle_set(ctx, subcmd_args),
            "append" => Self::handle_append(ctx, subcmd_args),
            "prepend" => Self::handle_prepend(ctx, subcmd_args),
            "reset" => Self::handle_reset(ctx),
            "file" => Self::handle_file(ctx, subcmd_args),
            _ => {
                ctx.app.push_system(
                    format!(
                        "Unknown subcommand '{}'. Available: show, set, append, prepend, reset, file",
                        subcmd
                    ),
                    true,
                );
            }
        }
    }
}

pub struct MemoryHandler;

impl MemoryHandler {
    /// Format a memory entry for display.
    fn format_memory_entry(entry: &crate::db::memory::MemoryEntry) -> String {
        let scope_label = match &entry.scope {
            crate::db::memory::MemoryScope::Global => "global".to_string(),
            crate::db::memory::MemoryScope::Project { path } => format!("project:{}", path),
        };
        let tags = if entry.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", entry.tags.join(", "))
        };
        format!("  `{}` ({}) {}{}\n", entry.id, scope_label, entry.text, tags)
    }

    fn handle_list(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore) {
        match mem.list(None) {
            Ok(entries) if entries.is_empty() => {
                ctx.app.push_system(
                    "No memories stored.\n\nUse `/memory add <text>` to save one.".to_string(),
                    false,
                );
            }
            Ok(entries) => {
                let mut out = format!("**Memories** ({} total):\n\n", entries.len());
                for e in &entries {
                    out.push_str(&Self::format_memory_entry(e));
                }
                out.push_str(
                    "\nUse `/memory edit <id> <new text>` to modify, `/memory remove <id>` to delete.",
                );
                ctx.app.push_system(out, false);
            }
            Err(e) => {
                ctx.app.push_system(format!("Failed to list memories: {}", e), true);
            }
        }
    }

    fn handle_add(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore, args: &str) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /memory add [--project] <text>".to_string(), true);
        } else {
            let (scope, text) = if args.starts_with("--project") {
                let rest = args.trim_start_matches("--project").trim();
                if rest.is_empty() {
                    ctx.app.push_system("Usage: /memory add --project <text>".to_string(), true);
                    return;
                }
                (
                    crate::db::memory::MemoryScope::Project { path: ctx.app.cwd.clone() },
                    rest.to_string(),
                )
            } else {
                (crate::db::memory::MemoryScope::Global, args.to_string())
            };

            let entry = crate::db::memory::MemoryEntry::new(&text, scope.clone())
                .with_source(crate::db::memory::MemorySource::User);
            let id = entry.id;
            match mem.save(&entry) {
                Ok(()) => {
                    ctx.app.push_system(
                        format!("Memory saved (id: `{}`, scope: {}):\n  {}", id, scope, text),
                        false,
                    );
                }
                Err(e) => {
                    ctx.app.push_system(format!("Failed to save memory: {}", e), true);
                }
            }
        }
    }

    fn handle_edit(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore, args: &str) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /memory edit <id> <new text>".to_string(), true);
        } else {
            let edit_parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let id_str = edit_parts[0].trim();
            let new_text = edit_parts.get(1).map(|s| s.trim()).unwrap_or("");

            if new_text.is_empty() {
                ctx.app.push_system("Usage: /memory edit <id> <new text>".to_string(), true);
            } else if let Ok(id) = id_str.parse::<u64>() {
                match mem.get(id) {
                    Ok(Some(mut entry)) => {
                        let old_text = entry.text.clone();
                        entry.text = new_text.to_string();
                        match mem.update(&entry) {
                            Ok(true) => {
                                ctx.app.push_system(
                                    format!(
                                        "Memory `{}` updated:\n  ~~{}~~\n  → {}",
                                        id, old_text, new_text
                                    ),
                                    false,
                                );
                            }
                            Ok(false) => {
                                ctx.app.push_system(format!("Memory `{}` not found.", id), true);
                            }
                            Err(e) => {
                                ctx.app.push_system(format!("Failed to update memory: {}", e), true);
                            }
                        }
                    }
                    Ok(None) => {
                        ctx.app.push_system(format!("No memory with id `{}`.", id), true);
                    }
                    Err(e) => {
                        ctx.app.push_system(format!("Failed to read memory: {}", e), true);
                    }
                }
            } else {
                ctx.app.push_system(
                    format!("Invalid memory ID: '{}'. Use `/memory list` to see IDs.", id_str),
                    true,
                );
            }
        }
    }

    fn handle_remove(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore, args: &str) {
        if args.is_empty() {
            ctx.app.push_system("Usage: /memory remove <id>".to_string(), true);
        } else if let Ok(id) = args.parse::<u64>() {
            match mem.remove(id) {
                Ok(true) => {
                    ctx.app.push_system(format!("Memory `{}` removed.", id), false);
                }
                Ok(false) => {
                    ctx.app.push_system(format!("No memory with id `{}`.", id), true);
                }
                Err(e) => {
                    ctx.app.push_system(format!("Failed to remove memory: {}", e), true);
                }
            }
        } else {
            ctx.app.push_system(
                format!("Invalid memory ID: '{}'. Use `/memory list` to see IDs.", args),
                true,
            );
        }
    }

    fn handle_search(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore, query: &str) {
        if query.is_empty() {
            ctx.app.push_system("Usage: /memory search <query>".to_string(), true);
        } else {
            match mem.search(query) {
                Ok(results) if results.is_empty() => {
                    ctx.app.push_system(format!("No memories matching '{}'.", query), false);
                }
                Ok(results) => {
                    let mut out = format!(
                        "**Search results** for '{}' ({} found):\n\n",
                        query,
                        results.len()
                    );
                    for e in &results {
                        out.push_str(&Self::format_memory_entry(e));
                    }
                    ctx.app.push_system(out, false);
                }
                Err(e) => {
                    ctx.app.push_system(format!("Search failed: {}", e), true);
                }
            }
        }
    }

    fn handle_clear(ctx: &mut SlashContext<'_>, mem: &crate::db::memory::MemoryStore) {
        match mem.clear() {
            Ok(count) => {
                ctx.app.push_system(format!("Cleared {} memory/memories.", count), false);
            }
            Err(e) => {
                ctx.app.push_system(format!("Failed to clear memories: {}", e), true);
            }
        }
    }
}

impl SlashHandler for MemoryHandler {
    fn command(&self) -> super::super::SlashCommand {
        super::super::SlashCommand {
            name: "memory",
            description: "Manage cross-session memory",
            help: "View, add, edit, remove, and search persistent memories.\n\n\
                   Usage:\n  \
                   /memory                   — list all memories\n  \
                   /memory add <text>         — add a global memory\n  \
                   /memory add --project <text> — add a project-scoped memory\n  \
                   /memory edit <id> <text>   — replace memory text by ID\n  \
                   /memory remove <id>        — remove a memory by ID\n  \
                   /memory search <query>     — search memories by text/tags\n  \
                   /memory clear              — remove all memories",
            accepts_args: true,
            subcommands: vec![
                ("add <text>", "add a global memory"),
                ("add --project <text>", "add a project-scoped memory"),
                ("edit <id> <text>", "replace memory text by ID"),
                ("remove <id>", "remove a memory by ID"),
                ("search <query>", "search memories"),
                ("clear", "remove all memories"),
            ],
            leader_key: None,
        }
    }

    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let Some(db) = &ctx.db else {
            ctx.app.push_system("Memory database not available (opened without --ctx.db).".to_string(), true);
            return;
        };

        let mem = db.memory();

        if args.is_empty() || args == "list" {
            Self::handle_list(ctx, &mem);
            return;
        }

        let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
        let subcmd = parts[0].trim();
        let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match subcmd {
            "add" => Self::handle_add(ctx, &mem, subcmd_args),
            "edit" | "update" => Self::handle_edit(ctx, &mem, subcmd_args),
            "remove" | "rm" | "delete" => Self::handle_remove(ctx, &mem, subcmd_args),
            "search" | "find" => Self::handle_search(ctx, &mem, subcmd_args),
            "clear" => Self::handle_clear(ctx, &mem),
            _ => {
                ctx.app.push_system(
                    format!(
                        "Unknown subcommand '{}'. Available: list, add, edit, remove, search, clear",
                        subcmd
                    ),
                    true,
                );
            }
        }
    }
}
