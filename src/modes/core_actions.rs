//! Core action handling — mode switching, editor movement, scrolling, etc.
//!
//! Extracted from event_handlers.rs to keep each function under 70 lines.

use std::sync::Arc;

use clankers_tui_types::AppState;

use crate::config::keybindings::CoreAction;
use crate::config::keybindings::InputMode;
use crate::tui::app::App;

/// Handle a resolved `CoreAction`.
pub(crate) fn handle_core_action(
    app: &mut App,
    action: CoreAction,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) {
    match action {
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
            handle_submit(app, cmd_tx, plugin_manager, panel_tx, db, session_manager, slash_registry);
        }
        CoreAction::NewLine => {
            app.editor.insert_char('\n');
        }
        CoreAction::Cancel => {
            handle_cancel(app, cmd_tx);
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
                app.input_mode = InputMode::Normal;
                app.slash_menu.hide();
            } else if app.conversation.focused_block.is_some() {
                app.conversation.focused_block = None;
                app.conversation.scroll.scroll_to_bottom();
            }
        }

        // ── Menu navigation (handled by slash menu intercept) ──
        CoreAction::MenuUp | CoreAction::MenuDown | CoreAction::MenuAccept | CoreAction::MenuClose => {}

        // ── Clipboard paste ──────────────────────────
        CoreAction::PasteImage => {
            crate::tui::clipboard::paste_from_clipboard(app);
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn handle_submit(
    app: &mut App,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    plugin_manager: Option<&Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    db: &Option<crate::db::Db>,
    session_manager: &mut Option<crate::session::SessionManager>,
    slash_registry: &crate::slash_commands::SlashRegistry,
) {
    if app.state != AppState::Idle {
        // Abort the current stream and queue the new prompt
        if let Some(text) = app.submit_input() {
            app.queued_prompt = Some(text);
            cmd_tx.send(super::interactive::AgentCommand::Abort).ok();
        }
        return;
    }
    if let Some(text) = app.submit_input() {
        if let Some((checkpoint, prompt)) = app.take_pending_branch(&text) {
            cmd_tx.send(super::interactive::AgentCommand::ResetCancel).ok();
            cmd_tx.send(super::interactive::AgentCommand::TruncateMessages(checkpoint)).ok();
            cmd_tx.send(super::interactive::AgentCommand::Prompt(prompt)).ok();
        } else {
            super::event_handlers::handle_input_with_plugins(
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

fn handle_cancel(app: &mut App, cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>) {
    if app.state == AppState::Streaming {
        cmd_tx.send(super::interactive::AgentCommand::Abort).ok();
    } else if !app.editor.is_empty() {
        app.editor.clear();
        app.slash_menu.hide();
    } else {
        app.should_quit = true;
    }
}
