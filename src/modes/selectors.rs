//! Popup selector key handling (model, account, session).

use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use crate::modes::interactive::AgentCommand;
use crate::tui::app::App;

pub(crate) fn handle_model_selector_key(
    app: &mut App,
    key: &crossterm::event::KeyEvent,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<AgentCommand>,
) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.overlays.model_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(model) = app.overlays.model_selector.select() {
                let old_model = std::mem::replace(&mut app.model, model.clone());
                let _ = cmd_tx.send(AgentCommand::SetModel(model.clone()));
                app.context_gauge.set_model(&app.model);
                app.push_system(format!("Model switched: {} → {}", old_model, model), false);
            }
            app.overlays.model_selector.close();
            true
        }
        KeyCode::Up => {
            app.overlays.model_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.overlays.model_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.overlays.model_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            // Ctrl+C closes
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.overlays.model_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.overlays.model_selector.move_up(),
                    'j' | 'n' => app.overlays.model_selector.move_down(),
                    _ => {}
                }
            } else {
                app.overlays.model_selector.type_char(c);
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
            app.overlays.account_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(account_name) = app.overlays.account_selector.select() {
                let _ = cmd_tx.send(AgentCommand::SwitchAccount(account_name));
            }
            app.overlays.account_selector.close();
            true
        }
        KeyCode::Up => {
            app.overlays.account_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.overlays.account_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.overlays.account_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.overlays.account_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.overlays.account_selector.move_up(),
                    'j' | 'n' => app.overlays.account_selector.move_down(),
                    _ => {}
                }
            } else {
                app.overlays.account_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Branch switcher key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_branch_switcher_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc => {
            app.branching.switcher.close();
            true
        }
        KeyCode::Enter => {
            if let Some(leaf_id) = app.branching.switcher.selected_leaf_id() {
                app.branching.switcher.close();
                app.switch_to_branch(leaf_id);
                app.push_system(format!("Switched to branch at block #{}", leaf_id), false);
            } else {
                app.branching.switcher.close();
            }
            true
        }
        KeyCode::Up => {
            app.branching.switcher.move_up();
            true
        }
        KeyCode::Down => {
            app.branching.switcher.move_down();
            true
        }
        KeyCode::Backspace => {
            app.branching.switcher.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.branching.switcher.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.branching.switcher.move_up(),
                    'j' | 'n' => app.branching.switcher.move_down(),
                    _ => {}
                }
            } else {
                app.branching.switcher.type_char(c);
            }
            true
        }
        _ => true,
    }
}

// ---------------------------------------------------------------------------
// Branch comparison key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_branch_compare_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.branching.compare.close();
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.branching.compare.scroll_down();
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.branching.compare.scroll_up();
            true
        }
        KeyCode::Left | KeyCode::Right | KeyCode::Char('h' | 'l') => {
            app.branching.compare.toggle_focus();
            true
        }
        KeyCode::Char('s') | KeyCode::Enter => {
            if let Some(leaf_id) = app.branching.compare.focused_leaf_id() {
                app.branching.compare.close();
                app.switch_to_branch(leaf_id);
                app.push_system(format!("Switched to branch at block #{}", leaf_id), false);
            }
            true
        }
        _ => true, // consume all keys while compare is open
    }
}

// ---------------------------------------------------------------------------
// Interactive merge key handling
// ---------------------------------------------------------------------------

pub(crate) fn handle_merge_interactive_key(app: &mut App, key: &crossterm::event::KeyEvent) -> bool {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.branching.merge_interactive.close();
            true
        }
        KeyCode::Char(' ') => {
            app.branching.merge_interactive.toggle();
            true
        }
        KeyCode::Char('a') => {
            app.branching.merge_interactive.select_all();
            true
        }
        KeyCode::Char('n') => {
            app.branching.merge_interactive.deselect_all();
            true
        }
        KeyCode::Char('j') | KeyCode::Down => {
            app.branching.merge_interactive.move_down();
            true
        }
        KeyCode::Char('k') | KeyCode::Up => {
            app.branching.merge_interactive.move_up();
            true
        }
        KeyCode::Enter => {
            if app.branching.merge_interactive.selected_count() > 0 {
                app.branching.merge_interactive.confirmed = true;
            } else {
                app.push_system("No messages selected for merge.".to_string(), true);
            }
            true
        }
        _ => true, // consume all keys while merge view is open
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
            app.overlays.session_selector.close();
            true
        }
        KeyCode::Enter => {
            if let Some(item) = app.overlays.session_selector.select() {
                let file_path = item.file_path.clone();
                let session_id = item.session_id.clone();
                app.overlays.session_selector.close();
                super::interactive::resume_session_from_file(app, file_path, &session_id, cmd_tx);
            } else {
                app.overlays.session_selector.close();
            }
            true
        }
        KeyCode::Up => {
            app.overlays.session_selector.move_up();
            true
        }
        KeyCode::Down => {
            app.overlays.session_selector.move_down();
            true
        }
        KeyCode::Backspace => {
            app.overlays.session_selector.backspace();
            true
        }
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) && c == 'c' {
                app.overlays.session_selector.close();
            } else if key.modifiers.contains(KeyModifiers::CONTROL) {
                match c {
                    'k' | 'p' => app.overlays.session_selector.move_up(),
                    'j' | 'n' => app.overlays.session_selector.move_down(),
                    _ => {}
                }
            } else {
                app.overlays.session_selector.type_char(c);
            }
            true
        }
        _ => true,
    }
}
