//! Tui slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct LayoutHandler;

impl SlashHandler for LayoutHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        use crate::tui::panes;

        let sub = args.trim().to_lowercase();
        match sub.as_str() {
            "default" | "3col" | "three" => {
                ctx.app.tiling = panes::default_tiling();
                ctx.app.pane_registry = panes::default_registry();
                ctx.app.unfocus_panel();
                ctx.app.push_system("Layout: default 3-column".into(), false);
            }
            "wide" | "chat" => {
                let (tiling, registry) = panes::wide_chat_tiling();
                ctx.app.tiling = tiling;
                ctx.app.pane_registry = registry;
                ctx.app.unfocus_panel();
                ctx.app.push_system("Layout: wide chat with left sidebar".into(), false);
            }
            "focused" | "none" | "clean" => {
                let (tiling, registry) = panes::focused_tiling();
                ctx.app.tiling = tiling;
                ctx.app.pane_registry = registry;
                ctx.app.unfocus_panel();
                ctx.app.push_system("Layout: focused (no panels)".into(), false);
            }
            "right" => {
                let (tiling, registry) = panes::right_heavy_tiling();
                ctx.app.tiling = tiling;
                ctx.app.pane_registry = registry;
                ctx.app.unfocus_panel();
                ctx.app.push_system("Layout: right-heavy".into(), false);
            }
            "" => {
                // Show current layout info
                let pane_count = ctx.app.tiling.panes().len();
                let panel_names: Vec<String> = ctx.app.tiling.panes().iter().filter_map(|p| {
                    match ctx.app.pane_registry.kind(p.id) {
                        Some(panes::PaneKind::Panel(panel_id)) => Some(panel_id.label().to_string()),
                        Some(panes::PaneKind::Chat) => Some("Chat".to_string()),
                        _ => None,
                    }
                }).collect();
                let msg = format!(
                    "Layout: {} pane(s): {}\nUse /layout <preset> to switch.\nPresets: default, wide, focused, right",
                    pane_count,
                    panel_names.join(", ")
                );
                ctx.app.push_system(msg, false);
            }
            _ => {
                ctx.app.push_system("Unknown layout. Use: default, wide, focused, right".into(), true);
            }
        }
    }
}

pub struct PreviewHandler;

impl SlashHandler for PreviewHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let content = if args.is_empty() {
            "# Markdown Preview\n\n\
             Here is some **bold text** and *italic text* and `inline code`.\n\n\
             ## Code Block\n\n\
             ```rust\n\
             fn main() {\n\
                 println!(\"Hello, world!\");\n\
             }\n\
             ```\n\n\
             ## Lists\n\n\
             - First item\n\
             - Second item\n\
             - Third item\n\n\
             1. Ordered one\n\
             2. Ordered two\n\n\
             > This is a blockquote\n\n\
             ---\n\n\
             A [link](https://example.com) and the end."
                .to_string()
        } else {
            args.to_string()
        };
        // Create a fake conversation block with the markdown as assistant text
        ctx.app.start_block("(markdown preview)".to_string(), 0);
        if let Some(ref mut block) = ctx.app.active_block {
            block.responses.push(crate::tui::app::DisplayMessage {
                role: crate::tui::app::MessageRole::Assistant,
                content,
                tool_name: None,
                is_error: false,
                images: Vec::new(),
            });
            block.streaming = false;
        }
        ctx.app.finalize_active_block();
        ctx.app.scroll.scroll_to_bottom();
    }
}

pub struct EditorHandler;

impl SlashHandler for EditorHandler {
    fn handle(&self, _args: &str, ctx: &mut SlashContext<'_>) {
        // Signal the event loop to open the external editor
        // (needs terminal access, which execute_slash_command doesn't have)
        ctx.app.open_editor_requested = true;
    }
}

pub struct TodoHandler;

impl SlashHandler for TodoHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        use crate::tui::components::todo_panel::{TodoPanel, TodoStatus};
        use crate::tui::panel::PanelId;

        if args.is_empty() {
            let summary = ctx.app.panels.downcast_ref::<TodoPanel>(PanelId::Todo)
                .expect("todo panel").summary();
            ctx.app.push_system(summary, false);
        } else {
            let todo_panel = ctx.app.panels.downcast_mut::<TodoPanel>(PanelId::Todo).expect("todo panel");
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "add" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo add <text>".to_string(), true);
                    } else {
                        let id = todo_panel.add(subcmd_args.to_string());
                        ctx.app.push_system(format!("Added todo #{}: {}", id, subcmd_args), false);
                    }
                }
                "done" | "complete" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo done <id or text>".to_string(), true);
                    } else if let Ok(id) = subcmd_args.parse::<usize>() {
                        if todo_panel.set_status(id, TodoStatus::Done) {
                            ctx.app.push_system(format!("Marked #{} as done.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else if let Some(id) = todo_panel.set_status_by_text(subcmd_args, TodoStatus::Done) {
                        ctx.app.push_system(format!("Marked #{} as done.", id), false);
                    } else {
                        ctx.app.push_system(format!("No todo matching '{}'.", subcmd_args), true);
                    }
                }
                "wip" | "active" | "start" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo wip <id or text>".to_string(), true);
                    } else if let Ok(id) = subcmd_args.parse::<usize>() {
                        if todo_panel.set_status(id, TodoStatus::InProgress) {
                            ctx.app.push_system(format!("Marked #{} as in-progress.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else if let Some(id) = todo_panel.set_status_by_text(subcmd_args, TodoStatus::InProgress)
                    {
                        ctx.app.push_system(format!("Marked #{} as in-progress.", id), false);
                    } else {
                        ctx.app.push_system(format!("No todo matching '{}'.", subcmd_args), true);
                    }
                }
                "remove" | "rm" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo remove <id>".to_string(), true);
                    } else if let Ok(id) = subcmd_args.parse::<usize>() {
                        if todo_panel.remove(id) {
                            ctx.app.push_system(format!("Removed todo #{}.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else {
                        ctx.app.push_system("Usage: /todo remove <id> (numeric ID required)".to_string(), true);
                    }
                }
                "clear" => {
                    todo_panel.clear_done();
                    ctx.app.push_system("Cleared completed items.".to_string(), false);
                }
                _ => {
                    // Treat bare text as "add"
                    let text = args.to_string();
                    let id = todo_panel.add(text.clone());
                    ctx.app.push_system(format!("Added todo #{}: {}", id, text), false);
                }
            }
        }
    }
}

pub struct PlanHandler;

impl SlashHandler for PlanHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let new_state = if args.eq_ignore_ascii_case("on") {
            crate::modes::plan::PlanState::Planning
        } else if args.eq_ignore_ascii_case("off") {
            crate::modes::plan::PlanState::Inactive
        } else {
            // Toggle
            if ctx.app.plan_state.is_active() {
                crate::modes::plan::PlanState::Inactive
            } else {
                crate::modes::plan::PlanState::Planning
            }
        };
        let msg = if new_state.is_active() {
            format!("{} Plan mode enabled — the agent will propose changes before editing.", new_state.emoji())
        } else {
            "Plan mode disabled — normal editing restored.".to_string()
        };
        ctx.app.plan_state = new_state;
        ctx.app.push_system(msg, false);
    }
}

pub struct ReviewHandler;

impl SlashHandler for ReviewHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        let base = if args.is_empty() { None } else { Some(args.as_str()) };
        let prompt = if let Some(b) = base {
            format!(
                "Please perform a thorough code review of the changes vs `{}`. \
                 Use the `review` tool with action='diff' first, then action='submit' \
                 with your findings.",
                b
            )
        } else {
            "Please perform a thorough code review of the recent changes. \
             Use the `review` tool with action='diff' first, then action='submit' \
             with your findings."
                .to_string()
        };
        let review_msg = if let Some(b) = base {
            format!("Starting code review vs {}...", b)
        } else {
            "Starting code review...".to_string()
        };
        ctx.app.push_system(review_msg, false);
        // Queue the review prompt to be sent as a user message
        ctx.app.queued_prompt = Some(prompt);
    }
}
