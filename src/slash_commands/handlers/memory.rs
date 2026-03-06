//! Memory slash command handlers.

use super::SlashContext;
use super::SlashHandler;
use crate::modes::interactive::AgentCommand;

pub struct SystemPromptHandler;

impl SlashHandler for SystemPromptHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if args.is_empty() || args == "show" {
            let full = args == "show";
            // Retrieve the current system prompt from the agent
            let (tx, mut rx) = tokio::sync::oneshot::channel();
            let _ = ctx.cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
            // We can't await here (sync fn), so spawn a task to receive and display
            let blocks_tx = {
                // We'll just show what we know: the original prompt.
                // The agent may have modified it, but we read it synchronously here.
                // For full accuracy we'd need async, but for display the oneshot
                // pattern below works within the event-loop tick.
                //
                // Instead, try_recv on the oneshot — the agent task processes
                // commands sequentially and may not have handled it yet.
                // Fall back to showing the original prompt.
                rx.try_recv().unwrap_or_else(|_| ctx.app.original_system_prompt.clone())
            };
            let prompt = blocks_tx;
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
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "set" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /system set <new system prompt text>".to_string(), true);
                    } else {
                        let new_prompt = subcmd_args.to_string();
                        let len = new_prompt.len();
                        let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                        ctx.app.push_system(
                            format!("System prompt replaced ({} chars). Takes effect on next message.", len),
                            false,
                        );
                    }
                }
                "append" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /system append <text to append>".to_string(), true);
                    } else {
                        // Read current prompt, append, and set
                        let (tx, mut rx) = tokio::sync::oneshot::channel();
                        let _ = ctx.cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                        let current = rx.try_recv().unwrap_or_else(|_| ctx.app.original_system_prompt.clone());
                        let new_prompt = format!("{}\n\n{}", current, subcmd_args);
                        let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                        ctx.app.push_system(
                            format!(
                                "Appended {} chars to system prompt. Takes effect on next message.",
                                subcmd_args.len()
                            ),
                            false,
                        );
                    }
                }
                "prepend" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /system prepend <text to prepend>".to_string(), true);
                    } else {
                        let (tx, mut rx) = tokio::sync::oneshot::channel();
                        let _ = ctx.cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                        let current = rx.try_recv().unwrap_or_else(|_| ctx.app.original_system_prompt.clone());
                        let new_prompt = format!("{}\n\n{}", subcmd_args, current);
                        let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(new_prompt));
                        ctx.app.push_system(
                            format!(
                                "Prepended {} chars to system prompt. Takes effect on next message.",
                                subcmd_args.len()
                            ),
                            false,
                        );
                    }
                }
                "reset" => {
                    let original = ctx.app.original_system_prompt.clone();
                    let len = original.len();
                    let _ = ctx.cmd_tx.send(AgentCommand::SetSystemPrompt(original));
                    ctx.app.push_system(
                        format!("System prompt reset to original ({} chars). Takes effect on next message.", len),
                        false,
                    );
                }
                "file" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /system file <path>".to_string(), true);
                    } else {
                        let path = if subcmd_args.starts_with('/') {
                            std::path::PathBuf::from(subcmd_args)
                        } else {
                            std::path::PathBuf::from(&ctx.app.cwd).join(subcmd_args)
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
                "show" => {
                    // Already handled above, but handle "show" as a subcmd too
                    let (tx, mut rx) = tokio::sync::oneshot::channel();
                    let _ = ctx.cmd_tx.send(AgentCommand::GetSystemPrompt(tx));
                    let prompt = rx.try_recv().unwrap_or_else(|_| ctx.app.original_system_prompt.clone());
                    ctx.app.push_system(format!("**System Prompt** ({} chars):\n\n{}", prompt.len(), prompt), false);
                }
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
}

pub struct MemoryHandler;

impl SlashHandler for MemoryHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        if let Some(db) = &ctx.db {
            let mem = db.memory();
            if args.is_empty() || args == "list" {
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
                            let scope_label = match &e.scope {
                                crate::db::memory::MemoryScope::Global => "global".to_string(),
                                crate::db::memory::MemoryScope::Project { path } => format!("project:{}", path),
                            };
                            let tags = if e.tags.is_empty() {
                                String::new()
                            } else {
                                format!(" [{}]", e.tags.join(", "))
                            };
                            out.push_str(&format!("  `{}` ({}) {}{}\n", e.id, scope_label, e.text, tags));
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
            } else {
                let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                let subcmd = parts[0].trim();
                let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

                match subcmd {
                    "add" => {
                        if subcmd_args.is_empty() {
                            ctx.app.push_system("Usage: /memory add [--project] <text>".to_string(), true);
                        } else {
                            let (scope, text) = if subcmd_args.starts_with("--project") {
                                let rest = subcmd_args.trim_start_matches("--project").trim();
                                if rest.is_empty() {
                                    ctx.app.push_system("Usage: /memory add --project <text>".to_string(), true);
                                    return;
                                }
                                (
                                    crate::db::memory::MemoryScope::Project { path: ctx.app.cwd.clone() },
                                    rest.to_string(),
                                )
                            } else {
                                (crate::db::memory::MemoryScope::Global, subcmd_args.to_string())
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
                    "edit" | "update" => {
                        if subcmd_args.is_empty() {
                            ctx.app.push_system("Usage: /memory edit <id> <new text>".to_string(), true);
                        } else {
                            let edit_parts: Vec<&str> = subcmd_args.splitn(2, char::is_whitespace).collect();
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
                    "remove" | "rm" | "delete" => {
                        if subcmd_args.is_empty() {
                            ctx.app.push_system("Usage: /memory remove <id>".to_string(), true);
                        } else if let Ok(id) = subcmd_args.parse::<u64>() {
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
                                format!("Invalid memory ID: '{}'. Use `/memory list` to see IDs.", subcmd_args),
                                true,
                            );
                        }
                    }
                    "search" | "find" => {
                        if subcmd_args.is_empty() {
                            ctx.app.push_system("Usage: /memory search <query>".to_string(), true);
                        } else {
                            match mem.search(subcmd_args) {
                                Ok(results) if results.is_empty() => {
                                    ctx.app.push_system(format!("No memories matching '{}'.", subcmd_args), false);
                                }
                                Ok(results) => {
                                    let mut out = format!(
                                        "**Search results** for '{}' ({} found):\n\n",
                                        subcmd_args,
                                        results.len()
                                    );
                                    for e in &results {
                                        let scope_label = match &e.scope {
                                            crate::db::memory::MemoryScope::Global => "global".to_string(),
                                            crate::db::memory::MemoryScope::Project { path } => {
                                                format!("project:{}", path)
                                            }
                                        };
                                        out.push_str(&format!("  `{}` ({}) {}\n", e.id, scope_label, e.text));
                                    }
                                    ctx.app.push_system(out, false);
                                }
                                Err(e) => {
                                    ctx.app.push_system(format!("Search failed: {}", e), true);
                                }
                            }
                        }
                    }
                    "clear" => match mem.clear() {
                        Ok(count) => {
                            ctx.app.push_system(format!("Cleared {} memory/memories.", count), false);
                        }
                        Err(e) => {
                            ctx.app.push_system(format!("Failed to clear memories: {}", e), true);
                        }
                    },
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
        } else {
            ctx.app.push_system("Memory database not available (opened without --ctx.db).".to_string(), true);
        }
    }
}
