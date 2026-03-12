//! Event handling functions for the TUI event loop.
//!
//! Dispatches key events, actions, and input routing. Heavy action handling
//! is delegated to:
//!   - `core_actions`     — CoreAction match dispatch
//!   - `extended_actions` — ExtendedAction match dispatch

use std::sync::Arc;

use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::config::keybindings::Action;
use crate::config::keybindings::ExtendedAction;
use crate::config::keybindings::InputMode;
use crate::config::keybindings::Keymap;
use crate::slash_commands;
use crate::tui::app::App;

// ---------------------------------------------------------------------------
// Action dispatcher
// ---------------------------------------------------------------------------

pub(crate) fn handle_action(
    app: &mut App,
    action: Action,
    _key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) {
    // When a panel is focused, intercept navigation actions and let
    // global actions (leader menu, selectors, etc.) fall through.
    if app.has_panel_focus() && handle_panel_focused_action(app, &action, cmd_tx) {
        return;
    }

    match action {
        Action::Core(core) => {
            super::core_actions::handle_core_action(
                app,
                core,
                cmd_tx,
                plugin_manager,
                panel_tx,
                db,
                session_manager,
                slash_registry,
            );
        }
        Action::Extended(ea) => {
            super::extended_actions::handle_extended_action(app, ea, cmd_tx, panel_tx);
        }
    }
}

/// When a panel is focused, intercept navigation actions. Returns true
/// if the action was consumed (caller should return early).
fn handle_panel_focused_action(
    app: &mut App,
    action: &Action,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
) -> bool {
    use crate::config::keybindings::CoreAction;
    use ratatui::layout::Direction;
    use ratatui_hypertile::HypertileAction;
    use ratatui_hypertile::Towards;

    let is_global = match action {
        Action::Core(c) => matches!(
            c,
            CoreAction::Quit | CoreAction::Cancel | CoreAction::EnterNormal | CoreAction::PasteImage
        ),
        Action::Extended(ea) => matches!(
            ea,
            ExtendedAction::OpenLeaderMenu
                | ExtendedAction::OpenModelSelector
                | ExtendedAction::OpenAccountSelector
                | ExtendedAction::ToggleThinking
                | ExtendedAction::ToggleShowThinking
                | ExtendedAction::ToggleBlockIds
                | ExtendedAction::SearchOutput
                | ExtendedAction::ToggleSessionPopup
                | ExtendedAction::ToggleBranchPanel
                | ExtendedAction::ToggleCostOverlay
                | ExtendedAction::OpenBranchSwitcher
                | ExtendedAction::OpenEditor
                | ExtendedAction::OpenToolToggle
                | ExtendedAction::TogglePromptImprove
                | ExtendedAction::ToggleAutoTest
        ),
    };

    if is_global {
        return false;
    }

    match action {
        Action::Core(CoreAction::Unfocus) | Action::Extended(ExtendedAction::TogglePanelFocus) => {
            app.close_focused_panel_views();
            app.zoom_restore();
            app.unfocus_panel();
            true
        }
        Action::Core(CoreAction::EnterInsert) => {
            app.close_focused_panel_views();
            app.zoom_restore();
            app.unfocus_panel();
            app.input_mode = InputMode::Insert;
            true
        }
        Action::Core(CoreAction::EnterCommand) => {
            app.close_focused_panel_views();
            app.zoom_restore();
            app.unfocus_panel();
            // Fall through to main handler for "/" prefix setup
            false
        }
        // h/l: directional focus horizontal
        Action::Extended(ExtendedAction::PanelNextTab | ExtendedAction::BranchNext) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Horizontal,
                towards: Towards::End,
            });
            true
        }
        Action::Extended(ExtendedAction::PanelPrevTab | ExtendedAction::BranchPrev) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Horizontal,
                towards: Towards::Start,
            });
            true
        }
        // j/k: directional focus vertical
        Action::Core(CoreAction::FocusPrevBlock) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Vertical,
                towards: Towards::Start,
            });
            true
        }
        Action::Core(CoreAction::FocusNextBlock) => {
            app.apply_tiling_action(HypertileAction::FocusDirection {
                direction: Direction::Vertical,
                towards: Towards::End,
            });
            true
        }
        // Everything else is consumed (don't leak to main handler)
        _ => {
            let _ = cmd_tx; // suppress unused warning
            true
        }
    }
}

// ---------------------------------------------------------------------------
// Leader menu action dispatch
// ---------------------------------------------------------------------------

pub(crate) fn handle_leader_action(
    app: &mut App,
    action: crate::tui::components::leader_menu::LeaderAction,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) {
    use clankers_tui_types::LeaderAction;

    match action {
        LeaderAction::KeymapAction(keymap_action) => {
            let dummy_key = crossterm::event::KeyEvent::new(KeyCode::Null, KeyModifiers::NONE);
            handle_action(
                app,
                keymap_action,
                &dummy_key,
                cmd_tx,
                plugin_manager,
                panel_tx,
                db,
                session_manager,
                slash_registry,
            );
        }
        LeaderAction::SlashCommand(command) => {
            handle_input_with_plugins(
                app,
                &command,
                cmd_tx,
                plugin_manager,
                panel_tx,
                db,
                session_manager,
                slash_registry,
            );
        }
        LeaderAction::Submenu(_) => {
            // Submenus are handled internally by LeaderMenu::handle_key
        }
    }
}

// ---------------------------------------------------------------------------
// Output search (Ctrl+F overlay)
// ---------------------------------------------------------------------------

pub(crate) fn handle_output_search_key(app: &mut App, key: &crossterm::event::KeyEvent) {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => {
            app.overlays.output_search.deactivate();
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.cancel();
        }
        (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.next_match();
            app.overlays.output_search.scroll_to_current = true;
        }
        (KeyCode::Enter, KeyModifiers::SHIFT) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.prev_match();
            app.overlays.output_search.scroll_to_current = true;
        }
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.toggle_mode();
            app.overlays.output_search.update_matches(&app.rendered_lines);
            app.overlays.output_search.scroll_to_current = true;
        }
        (KeyCode::Backspace, _) => {
            app.overlays.output_search.backspace();
            app.overlays.output_search.update_matches(&app.rendered_lines);
            app.overlays.output_search.scroll_to_current = true;
        }
        (KeyCode::Char(c), m) if m.is_empty() || m == KeyModifiers::SHIFT => {
            app.overlays.output_search.type_char(c);
            app.overlays.output_search.update_matches(&app.rendered_lines);
            app.overlays.output_search.scroll_to_current = true;
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Slash menu (insert mode only)
// ---------------------------------------------------------------------------

pub(crate) fn handle_slash_menu_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    keymap: &Keymap,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) -> bool {
    use crate::config::keybindings::CoreAction;

    if let Some(action) = keymap.resolve(InputMode::Insert, key) {
        match action {
            Action::Core(CoreAction::MenuUp | CoreAction::HistoryUp) => {
                app.slash_menu.select_prev();
                return true;
            }
            Action::Core(CoreAction::MenuDown | CoreAction::HistoryDown) => {
                app.slash_menu.select_next();
                return true;
            }
            Action::Core(CoreAction::MenuAccept) => {
                app.accept_slash_completion();
                app.update_slash_menu();
                return true;
            }
            Action::Core(CoreAction::MenuClose) => {
                app.slash_menu.hide();
                return true;
            }
            Action::Core(CoreAction::EnterNormal) => {
                app.slash_menu.hide();
                app.input_mode = InputMode::Normal;
                return true;
            }
            Action::Core(CoreAction::Submit) => {
                app.accept_slash_completion();
                if let Some(text) = app.submit_input() {
                    handle_input_with_plugins(
                        app, &text, cmd_tx, plugin_manager, panel_tx, db, session_manager, slash_registry,
                    );
                }
                return true;
            }
            Action::Core(CoreAction::DeleteBack) => {
                app.editor.delete_back();
                app.update_slash_menu();
                return true;
            }
            _ => return false,
        }
    }

    // Unmapped key — insert printable characters
    if let KeyCode::Char(c) = key.code
        && (key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT)
    {
        app.editor.insert_char(c);
        app.update_slash_menu();
        return true;
    }

    false
}

// ---------------------------------------------------------------------------
// Session popup
// ---------------------------------------------------------------------------

pub(crate) fn handle_session_popup_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    keymap: &Keymap,
) -> bool {
    use crate::config::keybindings::CoreAction;

    let action = keymap.resolve(app.input_mode, key);

    match action {
        Some(Action::Core(CoreAction::Unfocus | CoreAction::Quit)) => {
            app.overlays.session_popup_visible = false;
            true
        }
        Some(Action::Extended(ExtendedAction::ToggleSessionPopup)) => {
            app.overlays.session_popup_visible = false;
            true
        }
        Some(Action::Core(CoreAction::FocusPrevBlock)) => {
            app.focus_prev_block();
            true
        }
        Some(Action::Core(CoreAction::FocusNextBlock)) => {
            app.focus_next_block();
            true
        }
        Some(Action::Extended(ExtendedAction::BranchPrev)) => {
            app.branch_prev();
            true
        }
        Some(Action::Extended(ExtendedAction::BranchNext)) => {
            app.branch_next();
            true
        }
        Some(Action::Extended(ExtendedAction::ToggleBlockCollapse)) => {
            app.toggle_focused_block();
            true
        }
        Some(Action::Extended(ExtendedAction::CollapseAllBlocks)) => {
            app.collapse_all_blocks();
            true
        }
        Some(Action::Extended(ExtendedAction::ExpandAllBlocks)) => {
            app.expand_all_blocks();
            true
        }
        Some(Action::Extended(ExtendedAction::CopyBlock)) => {
            app.copy_focused_block();
            true
        }
        Some(Action::Core(CoreAction::ScrollToTop)) => {
            app.conversation.focused_block = app.conversation.blocks.iter().find_map(|e| match e {
                clankers_tui_types::BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        Some(Action::Core(CoreAction::ScrollToBottom)) => {
            app.conversation.focused_block = app.conversation.blocks.iter().rev().find_map(|e| match e {
                clankers_tui_types::BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        Some(Action::Core(CoreAction::EnterInsert | CoreAction::EnterCommand)) => {
            app.overlays.session_popup_visible = false;
            false // Let the main handler process it
        }
        _ => true, // Consume all other keys while popup is open
    }
}

// ---------------------------------------------------------------------------
// Character insertion (insert mode, unmapped keys)
// ---------------------------------------------------------------------------

pub(crate) fn handle_insert_char(app: &mut App, key: &crossterm::event::KeyEvent) {
    if let (KeyCode::Char(c), m) = (key.code, key.modifiers)
        && (m.is_empty() || m == KeyModifiers::SHIFT)
    {
        app.editor.insert_char(c);
        app.update_slash_menu();
    }
}

// ---------------------------------------------------------------------------
// Input routing
// ---------------------------------------------------------------------------

pub(crate) fn handle_input_with_plugins(
    app: &mut App,
    text: &str,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) {
    if let Some((command, args)) = slash_commands::parse_command(text) {
        let mut ctx = slash_commands::handlers::SlashContext {
            app,
            cmd_tx,
            plugin_manager,
            panel_tx,
            db,
            session_manager,
        };
        slash_registry.dispatch(&command, &args, &mut ctx);
    } else {
        let _ = cmd_tx.send(super::interactive::AgentCommand::ResetCancel);
        let mut pending_images = app.take_pending_images();

        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        let prompt_text = expanded.text;

        let at_file_images: Vec<crate::tui::app::PendingImage> = expanded
            .images
            .into_iter()
            .filter_map(|c| match c {
                crate::provider::message::Content::Image {
                    source: crate::provider::message::ImageSource::Base64 { media_type, data },
                } => {
                    let size = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, &data)
                        .map(|b| b.len())
                        .unwrap_or(0);
                    Some(crate::tui::app::PendingImage {
                        data,
                        media_type,
                        size,
                    })
                }
                _ => None,
            })
            .collect();
        pending_images.extend(at_file_images);

        if app.prompt_improve {
            if pending_images.is_empty() {
                let _ = cmd_tx.send(super::interactive::AgentCommand::RewriteAndPrompt(prompt_text));
            } else {
                let _ = cmd_tx.send(super::interactive::AgentCommand::RewriteAndPromptWithImages {
                    text: prompt_text,
                    images: pending_images,
                });
            }
        } else if pending_images.is_empty() {
            let _ = cmd_tx.send(super::interactive::AgentCommand::Prompt(prompt_text));
        } else {
            let _ = cmd_tx.send(super::interactive::AgentCommand::PromptWithImages {
                text: prompt_text,
                images: pending_images,
            });
        }
    }
}
