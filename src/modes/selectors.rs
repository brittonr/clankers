//! Popup selector key handling (model, account, session).

use crossterm::event::{KeyCode, KeyModifiers};

use crate::modes::interactive::AgentCommand;
use crate::tui::app::App;

pub(crate) fn handle_model_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.model_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(model) = app.model_selector.select() {
                let old_model = std::mem::replace(&mut app.model, model.clone());
                let _ = cmd_tx.send(AgentCommand::SetModel(model.clone()));
                app.context_gauge.set_model(&app.model);
                app.push_system(format!("Model switched: {} → {}", old_model, model), false);
            }
            app.model_selector.close();
            true
        }
        KeyCode::Up => {
            app.model_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.model_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.model_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            // Ctrl+C closes
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.model_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.model_selector.move_up(),
                    'j' | 'n' => app.model_selector.move_down(),
                    _ => {}
                }
            } else {
                app.model_selector.type_char(c);
            }
            true
        }
        _ => true, // consume all keys while selector is open
    }
}

// ---------------------------------------------------------------------------
// Account selector key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_account_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.account_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(account_name) = app.account_selector.select() {
                let _ = cmd_tx.send(AgentCommand::SwitchAccount(account_name));
            }
            app.account_selector.close();
            true
        }
        KeyCode::Up => {
            app.account_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.account_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.account_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.account_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.account_selector.move_up(),
                    'j' | 'n' => app.account_selector.move_down(),
                    _ => {}
                }
            } else {
                app.account_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Branch switcher key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_branch_switcher_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.branch_switcher.close();
            true
        }
        KeyCode::Enter => {
            if let Some(leaf_id) = app.branch_switcher.selected_leaf_id() {
                app.branch_switcher.close();
                app.switch_to_branch(leaf_id);
                app.push_system(format!("Switched to branch at block #{}", leaf_id), false);
            } else {
                app.branch_switcher.close();
            }
            true
        }
        KeyCode::Up => {
            app.branch_switcher.move_up();
            true
        }
        KeyCode::Down => {
            app.branch_switcher.move_down();
            true
        }
        KeyCode::Backspace => {
            app.branch_switcher.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.branch_switcher.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.branch_switcher.move_up(),
                    'j' | 'n' => app.branch_switcher.move_down(),
                    _ => {}
                }
            } else {
                app.branch_switcher.type_char(c);
            }
            true
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Branch comparison key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_branch_compare_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.branch_compare.close();
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.branch_compare.scroll_down();
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.branch_compare.scroll_up();
            true
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
            app.branch_compare.toggle_focus();
            true
        }
        KeyCode::Char('s') | KeyCode::Enter => {
            if let Some(leaf_id) = app.branch_compare.focused_leaf_id() {
                app.branch_compare.close();
                app.switch_to_branch(leaf_id);
                app.push_system(format!("Switched to branch at block #{}", leaf_id), false);
            }
            true
        }
        _ => true, // consume all keys while compare is open
    }
}

// ---------------------------------------------------------------------------
// Session selector key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_session_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.session_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(item) = app.session_selector.select() {
                let file_path = item.file_path.clone();
                let session_id = item.session_id.clone();
                app.session_selector.close();
                super::interactive::resume_session_from_file(app, file_path, &session_id, cmd_tx);
            } else {
                app.session_selector.close();
            }
            true
        }
        KeyCode::Up => {
            app.session_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.session_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.session_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.session_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.session_selector.move_up(),
                    'j' | 'n' => app.session_selector.move_down(),
                    _ => {}
                }
            } else {
                app.session_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}

