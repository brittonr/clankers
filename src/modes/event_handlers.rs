//! Event handling functions extracted from interactive.rs
//!
//! These handle key events, actions, and input routing for the TUI event loop.

use std::sync::Arc;

use clankers_tui_types::AppState;
use clankers_tui_types::BlockEntry;
use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::config::keybindings::Action;
use crate::config::keybindings::ExtendedAction;
use crate::config::keybindings::InputMode;
use crate::config::keybindings::Keymap;
use crate::provider::auth::AuthStoreExt;
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
    use crate::config::keybindings::CoreAction;

    // When a panel is focused, intercept navigation actions and let
    // global actions (leader menu, selectors, etc.) fall through.
    // Panel-specific key handling is done by Panel::handle_key_event()
    // in the raw key dispatch above — this block only handles Action-level
    // structural navigation (focus/unfocus, column movement, mode switching).
    if app.has_panel_focus() {
        let is_global = match &action {
            Action::Core(c) => {
                matches!(c, CoreAction::Quit | CoreAction::Cancel | CoreAction::EnterNormal | CoreAction::PasteImage)
            }
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
            ),
        };

        if !is_global {
            match &action {
                Action::Core(CoreAction::Unfocus) => {
                    app.close_focused_panel_views();
                    app.zoom_restore();
                    app.unfocus_panel();
                    return;
                }
                Action::Extended(ExtendedAction::TogglePanelFocus) => {
                    app.close_focused_panel_views();
                    app.zoom_restore();
                    app.unfocus_panel();
                    return;
                }
                Action::Core(CoreAction::EnterInsert) => {
                    app.close_focused_panel_views();
                    app.zoom_restore();
                    app.unfocus_panel();
                    app.input_mode = InputMode::Insert;
                    return;
                }
                Action::Core(CoreAction::EnterCommand) => {
                    app.close_focused_panel_views();
                    app.zoom_restore();
                    app.unfocus_panel();
                    // Don't return — fall through to main handler for "/" prefix setup
                }
                // h/l: use hypertile directional focus
                Action::Extended(ExtendedAction::PanelNextTab | ExtendedAction::BranchNext) => {
                    use ratatui::layout::Direction;
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: Direction::Horizontal,
                        towards: Towards::End,
                    });
                    return;
                }
                Action::Extended(ExtendedAction::PanelPrevTab | ExtendedAction::BranchPrev) => {
                    use ratatui::layout::Direction;
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: Direction::Horizontal,
                        towards: Towards::Start,
                    });
                    return;
                }
                // j/k: directional focus vertically
                Action::Core(CoreAction::FocusPrevBlock) => {
                    use ratatui::layout::Direction;
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: Direction::Vertical,
                        towards: Towards::Start,
                    });
                    return;
                }
                Action::Core(CoreAction::FocusNextBlock) => {
                    use ratatui::layout::Direction;
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: Direction::Vertical,
                        towards: Towards::End,
                    });
                    return;
                }
                // Everything else is consumed (don't leak to main handler)
                _ => return,
            }
        }
    }

    match action {
        // ── Core actions ────────────────────────────────
        Action::Core(core) => match core {
            // ── Mode switching ───────────────────────────
            CoreAction::EnterInsert => {
                app.input_mode = InputMode::Insert;
            }
            CoreAction::EnterCommand => {
                app.input_mode = InputMode::Insert;
                app.editor.clear();
                app.editor.insert_char('/');
                app.update_slash_menu();
            }
            CoreAction::EnterNormal => {
                app.input_mode = InputMode::Normal;
                app.slash_menu.hide();
            }

            // ── Core operations ──────────────────────────
            CoreAction::Submit => {
                if app.state != AppState::Idle {
                    // Abort the current stream and queue the new prompt
                    if let Some(text) = app.submit_input() {
                        app.queued_prompt = Some(text);
                        let _ = cmd_tx.send(super::interactive::AgentCommand::Abort);
                    }
                    return;
                }
                if let Some(text) = app.submit_input() {
                    if let Some((checkpoint, prompt)) = app.take_pending_branch(&text) {
                        let _ = cmd_tx.send(super::interactive::AgentCommand::ResetCancel);
                        let _ = cmd_tx.send(super::interactive::AgentCommand::TruncateMessages(checkpoint));
                        let _ = cmd_tx.send(super::interactive::AgentCommand::Prompt(prompt));
                    } else {
                        handle_input_with_plugins(
                            app,
                            &text,
                            cmd_tx,
                            plugin_manager,
                            panel_tx,
                            db,
                            session_manager,
                            slash_registry,
                        );
                    }
                }
            }
            CoreAction::NewLine => {
                app.editor.insert_char('\n');
            }
            CoreAction::Cancel => {
                if app.state == AppState::Streaming {
                    let _ = cmd_tx.send(super::interactive::AgentCommand::Abort);
                } else if !app.editor.is_empty() {
                    app.editor.clear();
                    app.slash_menu.hide();
                } else {
                    app.should_quit = true;
                }
            }
            CoreAction::Quit => {
                app.should_quit = true;
            }

            // ── Editor movement ──────────────────────────
            CoreAction::MoveLeft => app.editor.move_left(),
            CoreAction::MoveRight => app.editor.move_right(),
            CoreAction::MoveHome => app.editor.move_home(),
            CoreAction::MoveEnd => app.editor.move_end(),

            // ── Editor editing ───────────────────────────
            CoreAction::DeleteBack => {
                app.editor.delete_back();
                app.update_slash_menu();
            }
            CoreAction::DeleteForward => {
                app.editor.delete_forward();
                app.update_slash_menu();
            }
            CoreAction::DeleteWord => {
                app.editor.delete_word_back();
                app.update_slash_menu();
            }
            CoreAction::ClearLine => {
                app.editor.clear();
                app.slash_menu.hide();
            }

            // ── History ──────────────────────────────────
            CoreAction::HistoryUp => app.editor.history_up(),
            CoreAction::HistoryDown => app.editor.history_down(),

            // ── Scrolling ────────────────────────────────
            CoreAction::ScrollUp => app.conversation.scroll.scroll_up(1),
            CoreAction::ScrollDown => app.conversation.scroll.scroll_down(1),
            CoreAction::ScrollPageUp => app.conversation.scroll.scroll_up(10),
            CoreAction::ScrollPageDown => app.conversation.scroll.scroll_down(10),
            CoreAction::ScrollToTop => app.conversation.scroll.scroll_to_top(),
            CoreAction::ScrollToBottom => app.conversation.scroll.scroll_to_bottom(),

            // ── Block navigation ─────────────────────────
            CoreAction::FocusPrevBlock => app.focus_prev_block(),
            CoreAction::FocusNextBlock => app.focus_next_block(),
            CoreAction::Unfocus => {
                if app.input_mode == InputMode::Insert {
                    // Esc in insert → normal
                    app.input_mode = InputMode::Normal;
                    app.slash_menu.hide();
                } else if app.conversation.focused_block.is_some() {
                    app.conversation.focused_block = None;
                    app.conversation.scroll.scroll_to_bottom();
                }
            }

            // ── Menu navigation ──────────────────────────
            CoreAction::MenuUp | CoreAction::MenuDown | CoreAction::MenuAccept | CoreAction::MenuClose => {
                // Menu actions are handled by handle_slash_menu_key before reaching here
            }

            // ── Clipboard paste ──────────────────────────
            CoreAction::PasteImage => {
                crate::tui::clipboard::paste_from_clipboard(app);
            }
        },

        // ── Extended actions ────────────────────────────
        Action::Extended(ea) => match ea {
            // ── Search ───────────────────────────────────
            ExtendedAction::SearchOutput => {
                app.overlays.output_search.activate();
            }
            ExtendedAction::SearchNext => {
                if !app.overlays.output_search.matches.is_empty() {
                    app.overlays.output_search.next_match();
                    app.overlays.output_search.scroll_to_current = true;
                }
            }
            ExtendedAction::SearchPrev => {
                if !app.overlays.output_search.matches.is_empty() {
                    app.overlays.output_search.prev_match();
                    app.overlays.output_search.scroll_to_current = true;
                }
            }

            // ── Block operations ─────────────────────────
            ExtendedAction::ToggleBlockCollapse => {
                if app.conversation.focused_block.is_some() {
                    app.toggle_focused_block();
                }
            }
            ExtendedAction::CollapseAllBlocks => app.collapse_all_blocks(),
            ExtendedAction::ExpandAllBlocks => app.expand_all_blocks(),
            ExtendedAction::CopyBlock => app.copy_focused_block(),
            ExtendedAction::RerunBlock => {
                if let Some(prompt) = app.get_focused_block_prompt() {
                    let _ = cmd_tx.send(super::interactive::AgentCommand::ResetCancel);
                    let _ = cmd_tx.send(super::interactive::AgentCommand::Prompt(prompt));
                }
            }
            ExtendedAction::EditBlock => {
                if app.conversation.focused_block.is_some()
                    && app.state == AppState::Idle
                    && app.edit_focused_block_prompt()
                {
                    app.input_mode = InputMode::Insert;
                }
            }

            // ── Branch / panel navigation ────────────────
            ExtendedAction::BranchPrev => {
                if app.conversation.focused_block.is_some() {
                    app.branch_prev();
                } else {
                    // h = directional focus left
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: ratatui::layout::Direction::Horizontal,
                        towards: Towards::Start,
                    });
                    app.input_mode = InputMode::Normal;
                }
            }
            ExtendedAction::BranchNext => {
                if app.conversation.focused_block.is_some() {
                    app.branch_next();
                } else {
                    // l = directional focus right
                    use ratatui_hypertile::HypertileAction;
                    use ratatui_hypertile::Towards;
                    app.apply_tiling_action(HypertileAction::FocusDirection {
                        direction: ratatui::layout::Direction::Horizontal,
                        towards: Towards::End,
                    });
                    app.input_mode = InputMode::Normal;
                }
            }

            // ── Toggles ─────────────────────────────────
            ExtendedAction::ToggleThinking => {
                let _ = cmd_tx.send(super::interactive::AgentCommand::CycleThinkingLevel);
            }
            ExtendedAction::ToggleShowThinking => {
                app.show_thinking = !app.show_thinking;
                let state = if app.show_thinking { "visible" } else { "hidden" };
                app.push_system(format!("Thinking content now {}.", state), false);
            }
            ExtendedAction::ToggleBlockIds => {
                app.overlays.show_block_ids = !app.overlays.show_block_ids;
                let state = if app.overlays.show_block_ids {
                    "visible"
                } else {
                    "hidden"
                };
                app.push_system(format!("Block IDs now {}.", state), false);
            }

            // ── Panel focus ─────────────────────────────
            ExtendedAction::TogglePanelFocus => {
                if app.has_panel_focus() {
                    app.unfocus_panel();
                } else {
                    // Focus the next pane (cycles through all panes)
                    use ratatui_hypertile::HypertileAction;
                    app.apply_tiling_action(HypertileAction::FocusNext);
                    app.input_mode = InputMode::Normal;
                }
            }
            ExtendedAction::PanelNextTab => {
                // l = directional focus right
                use ratatui_hypertile::HypertileAction;
                use ratatui_hypertile::Towards;
                app.apply_tiling_action(HypertileAction::FocusDirection {
                    direction: ratatui::layout::Direction::Horizontal,
                    towards: Towards::End,
                });
                app.input_mode = InputMode::Normal;
            }
            ExtendedAction::PanelPrevTab => {
                // h = directional focus left
                use ratatui_hypertile::HypertileAction;
                use ratatui_hypertile::Towards;
                app.apply_tiling_action(HypertileAction::FocusDirection {
                    direction: ratatui::layout::Direction::Horizontal,
                    towards: Towards::Start,
                });
                app.input_mode = InputMode::Normal;
            }
            // ── Pane tiling actions (from leader menu) ─
            ExtendedAction::PaneSplitVertical => {
                app.split_focused_pane(ratatui::layout::Direction::Vertical);
            }
            ExtendedAction::PaneSplitHorizontal => {
                app.split_focused_pane(ratatui::layout::Direction::Horizontal);
            }
            ExtendedAction::PaneClose => {
                app.close_focused_pane();
            }
            ExtendedAction::PaneEqualize => {
                use ratatui_hypertile::HypertileAction;
                app.apply_tiling_action(HypertileAction::SetFocusedRatio { ratio: 0.5 });
            }
            ExtendedAction::PaneGrow => {
                use ratatui_hypertile::HypertileAction;
                app.apply_tiling_action(HypertileAction::ResizeFocused { delta: 0.05 });
            }
            ExtendedAction::PaneShrink => {
                use ratatui_hypertile::HypertileAction;
                app.apply_tiling_action(HypertileAction::ResizeFocused { delta: -0.05 });
            }
            ExtendedAction::PaneMoveLeft => {
                move_focused_pane(app, ratatui::layout::Direction::Horizontal, ratatui_hypertile::Towards::Start);
            }
            ExtendedAction::PaneMoveRight => {
                move_focused_pane(app, ratatui::layout::Direction::Horizontal, ratatui_hypertile::Towards::End);
            }
            ExtendedAction::PaneMoveDown => {
                move_focused_pane(app, ratatui::layout::Direction::Vertical, ratatui_hypertile::Towards::End);
            }
            ExtendedAction::PaneMoveUp => {
                move_focused_pane(app, ratatui::layout::Direction::Vertical, ratatui_hypertile::Towards::Start);
            }
            ExtendedAction::PaneZoom => {
                app.zoom_toggle();
            }
            ExtendedAction::PanelScrollUp => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(sp) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
                    sp.scroll.scroll_up(3);
                }
            }
            ExtendedAction::PanelScrollDown => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(sp) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
                    sp.scroll.scroll_down(3);
                }
            }
            ExtendedAction::PanelClearDone => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(subagent_panel) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
                    subagent_panel.clear_done();
                    if !subagent_panel.is_visible() {
                        app.unfocus_panel();
                    }
                }
            }
            ExtendedAction::PanelKill => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(sp) = app.panels.downcast_ref::<SubagentPanel>(PanelId::Subagents)
                    && let Some(id) = sp.selected_id()
                {
                    let _ = panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest { id });
                }
            }
            ExtendedAction::PanelRemove => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(sp) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
                    sp.remove_selected();
                }
            }

            // ── Cost overlay ─────────────────────────────
            ExtendedAction::ToggleCostOverlay => {
                app.overlays.cost_overlay_visible = !app.overlays.cost_overlay_visible;
            }

            // ── Session popup ─────────────────────────────
            ExtendedAction::ToggleSessionPopup => {
                app.overlays.session_popup_visible = !app.overlays.session_popup_visible;
                if app.overlays.session_popup_visible {
                    // Focus the last block when opening so user can navigate
                    if app.conversation.focused_block.is_none() {
                        let last_id = app.conversation.blocks.iter().rev().find_map(|e| match e {
                            BlockEntry::Conversation(b) => Some(b.id),
                            _ => None,
                        });
                        app.conversation.focused_block = last_id;
                    }
                }
            }

            // ── Branch panel ──────────────────────────────
            ExtendedAction::ToggleBranchPanel => {
                use clankers_tui_types::PanelId;

                use crate::tui::components::branch_panel::BranchPanel;
                if app.layout.focused_panel == Some(PanelId::Branches) {
                    // Unfocus (panel stays in the tree but we leave it)
                    app.unfocus_panel();
                } else {
                    // Refresh branch data and focus it
                    let active_ids: std::collections::HashSet<usize> = app
                        .conversation
                        .blocks
                        .iter()
                        .filter_map(|e| match e {
                            BlockEntry::Conversation(b) => Some(b.id),
                            _ => None,
                        })
                        .collect();
                    if let Some(bp) = app.panels.downcast_mut::<BranchPanel>(PanelId::Branches) {
                        bp.refresh(&app.conversation.all_blocks.clone(), &active_ids);
                    }
                    app.focus_panel(PanelId::Branches);
                }
            }

            // ── Branch switcher ─────────────────────────────
            ExtendedAction::OpenBranchSwitcher => {
                let active_ids: std::collections::HashSet<usize> = app
                    .conversation
                    .blocks
                    .iter()
                    .filter_map(|e| match e {
                        BlockEntry::Conversation(b) => Some(b.id),
                        _ => None,
                    })
                    .collect();
                app.branching.switcher.open(&app.conversation.all_blocks.clone(), &active_ids);
            }

            // ── External editor ─────────────────────────
            ExtendedAction::OpenEditor => {
                // Handled specially in the event loop (needs terminal access)
                // This is a marker — the event loop checks for it after handle_action
            }

            // ── Selectors ───────────────────────────────
            ExtendedAction::OpenModelSelector => {
                let models = app.available_models.clone();
                if models.is_empty() {
                    app.push_system("No models available.".to_string(), true);
                } else {
                    app.overlays.model_selector = crate::tui::components::model_selector::ModelSelector::new(models);
                    app.overlays.model_selector.open();
                }
            }
            ExtendedAction::OpenAccountSelector => {
                let paths = crate::config::ClankersPaths::get();
                let store = crate::provider::auth::AuthStore::load(&paths.global_auth);
                let accounts: Vec<crate::tui::components::account_selector::AccountItem> = store
                    .list_anthropic_accounts()
                    .into_iter()
                    .map(|info| crate::tui::components::account_selector::AccountItem {
                        name: info.name,
                        label: info.label,
                        is_active: info.is_active,
                        is_expired: info.is_expired,
                    })
                    .collect();
                if accounts.is_empty() {
                    app.push_system("No accounts configured. Use /login to authenticate.".to_string(), true);
                } else {
                    app.overlays.account_selector.open(accounts);
                }
            }

            // ── Leader key ──────────────────────────────
            ExtendedAction::OpenLeaderMenu => {
                app.overlays.leader_menu.open();
            }

            // Remaining extended actions handled elsewhere (tiling, etc.)
            _ => {}
        },
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
            // Re-use the existing action dispatcher with a dummy key event
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
            // Execute as if the user typed and submitted the slash command
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
    use crossterm::event::KeyCode;
    use crossterm::event::KeyModifiers;

    match (key.code, key.modifiers) {
        // Close search
        (KeyCode::Esc, _) => {
            app.overlays.output_search.deactivate();
        }
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.cancel();
        }

        // Navigate matches
        (KeyCode::Enter, KeyModifiers::NONE) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.next_match();
            app.overlays.output_search.scroll_to_current = true;
        }
        (KeyCode::Enter, KeyModifiers::SHIFT) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.prev_match();
            app.overlays.output_search.scroll_to_current = true;
        }

        // Toggle search mode (substring ↔ fuzzy)
        (KeyCode::Char('r'), KeyModifiers::CONTROL) => {
            app.overlays.output_search.toggle_mode();
            // Recompute matches immediately with new mode
            app.overlays.output_search.update_matches(&app.rendered_lines);
            app.overlays.output_search.scroll_to_current = true;
        }

        // Edit query
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

        // Consume all other keys (don't leak to main handler)
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

    // Resolve through the keymap — menu actions take priority when menu is visible
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
                        app,
                        &text,
                        cmd_tx,
                        plugin_manager,
                        panel_tx,
                        db,
                        session_manager,
                        slash_registry,
                    );
                }
                return true;
            }
            Action::Core(CoreAction::DeleteBack) => {
                app.editor.delete_back();
                app.update_slash_menu();
                return true;
            }
            // Other mapped actions — fall through to main handler
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
// Session popup (key handling when visible)
// ---------------------------------------------------------------------------

pub(crate) fn handle_session_popup_key(app: &mut App, key: &crossterm::event::KeyEvent, keymap: &Keymap) -> bool {
    use crate::config::keybindings::CoreAction;

    // Resolve through the current mode's keymap
    let action = keymap.resolve(app.input_mode, key);

    match action {
        // Close on Esc, 's' toggle, or 'q'
        Some(Action::Core(CoreAction::Unfocus | CoreAction::Quit)) => {
            app.overlays.session_popup_visible = false;
            true
        }
        Some(Action::Extended(ExtendedAction::ToggleSessionPopup)) => {
            app.overlays.session_popup_visible = false;
            true
        }
        // Navigate blocks with j/k
        Some(Action::Core(CoreAction::FocusPrevBlock)) => {
            app.focus_prev_block();
            true
        }
        Some(Action::Core(CoreAction::FocusNextBlock)) => {
            app.focus_next_block();
            true
        }
        // Branch navigation with h/l
        Some(Action::Extended(ExtendedAction::BranchPrev)) => {
            app.branch_prev();
            true
        }
        Some(Action::Extended(ExtendedAction::BranchNext)) => {
            app.branch_next();
            true
        }
        // Collapse/expand
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
        // Copy focused block
        Some(Action::Extended(ExtendedAction::CopyBlock)) => {
            app.copy_focused_block();
            true
        }
        // Scroll to top/bottom
        Some(Action::Core(CoreAction::ScrollToTop)) => {
            app.conversation.focused_block = app.conversation.blocks.iter().find_map(|e| match e {
                BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        Some(Action::Core(CoreAction::ScrollToBottom)) => {
            app.conversation.focused_block = app.conversation.blocks.iter().rev().find_map(|e| match e {
                BlockEntry::Conversation(b) => Some(b.id),
                _ => None,
            });
            true
        }
        // Switch to insert mode closes popup
        Some(Action::Core(CoreAction::EnterInsert | CoreAction::EnterCommand)) => {
            app.overlays.session_popup_visible = false;
            // Don't consume — let the main handler process it
            false
        }
        // All other keys are consumed (don't pass through while popup is open)
        _ => true,
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

        // Expand @file references — text files are inlined, images become Content blocks
        let expanded = crate::util::at_file::expand_at_refs_with_images(text, &app.cwd);
        let prompt_text = expanded.text;

        // Convert @file images into PendingImage and merge with clipboard-pasted images
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
                    Some(crate::tui::app::PendingImage { data, media_type, size })
                }
                _ => None,
            })
            .collect();
        pending_images.extend(at_file_images);

        if pending_images.is_empty() {
            let _ = cmd_tx.send(super::interactive::AgentCommand::Prompt(prompt_text));
        } else {
            let _ = cmd_tx.send(super::interactive::AgentCommand::PromptWithImages {
                text: prompt_text,
                images: pending_images,
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Tiling helpers
// ---------------------------------------------------------------------------

fn move_focused_pane(app: &mut App, direction: ratatui::layout::Direction, towards: ratatui_hypertile::Towards) {
    app.apply_tiling_action(ratatui_hypertile::HypertileAction::MoveFocused {
        direction,
        towards,
        scope: ratatui_hypertile::MoveScope::Window,
    });
}
