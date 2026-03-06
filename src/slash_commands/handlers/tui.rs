//! Tui slash command handlers.

use super::SlashContext;
use super::SlashHandler;

pub struct LayoutHandler;

impl SlashHandler for LayoutHandler {
    fn handle(&self, args: &str, ctx: &mut SlashContext<'_>) {
        use crate::tui::layout::PanelLayout;
        use crate::tui::panel::PanelId;

        let sub = args.trim().to_lowercase();
        match sub.as_str() {
            "default" | "3col" | "three" => {
                ctx.app.panel_layout = PanelLayout::default_three_column();
                ctx.app.push_system("Layout: default 3-column".into(), false);
            }
            "wide" | "chat" => {
                ctx.app.panel_layout = PanelLayout::wide_chat();
                ctx.app.push_system("Layout: wide chat with left sidebar".into(), false);
            }
            "focused" | "none" | "clean" => {
                ctx.app.panel_layout = PanelLayout::focused();
                ctx.app.panel_focused = false;
                ctx.app.focus.unfocus();
                ctx.app.push_system("Layout: focused (no panels)".into(), false);
            }
            "right" => {
                ctx.app.panel_layout = PanelLayout::right_heavy();
                ctx.app.push_system("Layout: right-heavy".into(), false);
            }
            s if s.starts_with("toggle ") => {
                let panel_name = s.trim_start_matches("toggle ").trim();
                let panel_id = match panel_name {
                    "todo" => Some(PanelId::Todo),
                    "files" | "file" => Some(PanelId::Files),
                    "subagents" | "sub" => Some(PanelId::Subagents),
                    "peers" | "peer" => Some(PanelId::Peers),
                    _ => None,
                };
                if let Some(id) = panel_id {
                    ctx.app.panel_layout.toggle_panel(id);
                    ctx.app.push_system(format!("Toggled panel: {}", id.label()), false);
                } else {
                    ctx.app.push_system(
                        format!("Unknown panel '{}'. Use: todo, files, subagents, peers", panel_name),
                        true,
                    );
                }
            }
            "" => {
                // Show current layout info
                let order = ctx.app.panel_layout.focus_order();
                let names: Vec<&str> = order.iter().map(|id| id.label()).collect();
                let msg = if names.is_empty() {
                    "Layout: focused (no panels)\nUse /layout <preset> to switch.\nPresets: default, wide, focused, right".to_string()
                } else {
                    format!(
                        "Layout: {} panel(s) visible: {}\nUse /layout <preset> to switch.\nPresets: default, wide, focused, right",
                        names.len(),
                        names.join(", ")
                    )
                };
                ctx.app.push_system(msg, false);
            }
            _ => {
                ctx.app.push_system("Unknown layout. Use: default, wide, focused, right, toggle <panel>".into(), true);
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
        use crate::tui::components::todo_panel::TodoStatus;

        if args.is_empty() {
            ctx.app.push_system(ctx.app.todo_panel.summary(), false);
        } else {
            let parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
            let subcmd = parts[0].trim();
            let subcmd_args = parts.get(1).map(|s| s.trim()).unwrap_or("");

            match subcmd {
                "add" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo add <text>".to_string(), true);
                    } else {
                        let id = ctx.app.todo_panel.add(subcmd_args.to_string());
                        ctx.app.push_system(format!("Added todo #{}: {}", id, subcmd_args), false);
                    }
                }
                "done" | "complete" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo done <id or text>".to_string(), true);
                    } else if let Ok(id) = subcmd_args.parse::<usize>() {
                        if ctx.app.todo_panel.set_status(id, TodoStatus::Done) {
                            ctx.app.push_system(format!("Marked #{} as done.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else if let Some(id) = ctx.app.todo_panel.set_status_by_text(subcmd_args, TodoStatus::Done) {
                        ctx.app.push_system(format!("Marked #{} as done.", id), false);
                    } else {
                        ctx.app.push_system(format!("No todo matching '{}'.", subcmd_args), true);
                    }
                }
                "wip" | "active" | "start" => {
                    if subcmd_args.is_empty() {
                        ctx.app.push_system("Usage: /todo wip <id or text>".to_string(), true);
                    } else if let Ok(id) = subcmd_args.parse::<usize>() {
                        if ctx.app.todo_panel.set_status(id, TodoStatus::InProgress) {
                            ctx.app.push_system(format!("Marked #{} as in-progress.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else if let Some(id) = ctx.app.todo_panel.set_status_by_text(subcmd_args, TodoStatus::InProgress)
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
                        if ctx.app.todo_panel.remove(id) {
                            ctx.app.push_system(format!("Removed todo #{}.", id), false);
                        } else {
                            ctx.app.push_system(format!("No todo item #{}.", id), true);
                        }
                    } else {
                        ctx.app.push_system("Usage: /todo remove <id> (numeric ID required)".to_string(), true);
                    }
                }
                "clear" => {
                    ctx.app.todo_panel.clear_done();
                    ctx.app.push_system("Cleared completed items.".to_string(), false);
                }
                _ => {
                    // Treat bare text as "add"
                    let text = args.to_string();
                    let id = ctx.app.todo_panel.add(text.clone());
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
