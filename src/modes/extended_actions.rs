//! Extended action handling — search, blocks, panels, tiling, selectors, etc.
//!
//! Extracted from event_handlers.rs to keep each function under 70 lines.

use clankers_tui_types::AppState;
use clankers_tui_types::BlockEntry;

use crate::config::keybindings::ExtendedAction;
use crate::config::keybindings::InputMode;
use crate::tui::app::App;

/// Handle a resolved `ExtendedAction`.
pub(crate) fn handle_extended_action(
    app: &mut App,
    action: ExtendedAction,
    cmd_tx: &tokio::sync::mpsc::UnboundedSender<super::interactive::AgentCommand>,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    match action {
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
        ExtendedAction::BranchPrev => handle_branch_prev(app),
        ExtendedAction::BranchNext => handle_branch_next(app),

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
        ExtendedAction::TogglePanelFocus => handle_toggle_panel_focus(app),
        ExtendedAction::PanelNextTab => handle_directional_focus(app, ratatui_hypertile::Towards::End),
        ExtendedAction::PanelPrevTab => handle_directional_focus(app, ratatui_hypertile::Towards::Start),

        // ── Pane tiling actions ─────────────────────
        ExtendedAction::PaneSplitVertical => {
            app.split_focused_pane(ratatui::layout::Direction::Vertical);
        }
        ExtendedAction::PaneSplitHorizontal => {
            app.split_focused_pane(ratatui::layout::Direction::Horizontal);
        }
        ExtendedAction::PaneClose => app.close_focused_pane(),
        ExtendedAction::PaneEqualize => {
            app.apply_tiling_action(ratatui_hypertile::HypertileAction::SetFocusedRatio { ratio: 0.5 });
        }
        ExtendedAction::PaneGrow => {
            app.apply_tiling_action(ratatui_hypertile::HypertileAction::ResizeFocused { delta: 0.05 });
        }
        ExtendedAction::PaneShrink => {
            app.apply_tiling_action(ratatui_hypertile::HypertileAction::ResizeFocused { delta: -0.05 });
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
        ExtendedAction::PaneZoom => app.zoom_toggle(),
        ExtendedAction::PanelScrollUp => handle_panel_scroll(app, true),
        ExtendedAction::PanelScrollDown => handle_panel_scroll(app, false),
        ExtendedAction::PanelClearDone => handle_panel_clear_done(app),
        ExtendedAction::PanelKill => handle_panel_kill(app, panel_tx),
        ExtendedAction::PanelRemove => handle_panel_remove(app),

        // ── Cost overlay ─────────────────────────────
        ExtendedAction::ToggleCostOverlay => {
            app.overlays.cost_overlay_visible = !app.overlays.cost_overlay_visible;
        }

        // ── Session popup ────────────────────────────
        ExtendedAction::ToggleSessionPopup => handle_toggle_session_popup(app),

        // ── Branch panel ─────────────────────────────
        ExtendedAction::ToggleBranchPanel => handle_toggle_branch_panel(app),

        // ── Branch switcher ──────────────────────────
        ExtendedAction::OpenBranchSwitcher => {
            let active_ids = collect_active_block_ids(app);
            app.branching.switcher.open(&app.conversation.all_blocks.clone(), &active_ids);
        }

        // ── External editor ──────────────────────────
        ExtendedAction::OpenEditor => {
            // Marker — the event loop handles this after handle_action
        }

        // ── Selectors ───────────────────────────────
        ExtendedAction::OpenModelSelector => handle_open_model_selector(app),
        ExtendedAction::OpenAccountSelector => handle_open_account_selector(app),

        // ── Leader key ──────────────────────────────
        ExtendedAction::OpenLeaderMenu => {
            app.overlays.leader_menu.open();
        }

        // ── Tool toggle ────────────────────────────
        ExtendedAction::OpenToolToggle => {
            let tools = app.tool_info.clone();
            app.overlays.tool_toggle.open(tools, &app.disabled_tools);
        }

        // ── Prompt improve toggle ────────────────────
        ExtendedAction::TogglePromptImprove => {
            app.prompt_improve = !app.prompt_improve;
            let state = if app.prompt_improve { "on" } else { "off" };
            app.push_system(format!("Prompt improve: {}.", state), false);
        }

        // ── Auto-test toggle ────────────────────────
        ExtendedAction::ToggleAutoTest => {
            if app.auto_test_command.is_none() {
                app.push_system(
                    "No test command configured. Set \"autoTestCommand\" in settings.json.".to_string(),
                    true,
                );
            } else {
                app.auto_test_enabled = !app.auto_test_enabled;
                let state = if app.auto_test_enabled { "on" } else { "off" };
                app.push_system(
                    format!("Auto-test {}: {}", state, app.auto_test_command.as_deref().unwrap_or("(none)")),
                    false,
                );
            }
        }

        // Remaining extended actions handled elsewhere (tiling, etc.)
        _ => {}
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn handle_branch_prev(app: &mut App) {
    if app.conversation.focused_block.is_some() {
        app.branch_prev();
    } else {
        app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusDirection {
            direction: ratatui::layout::Direction::Horizontal,
            towards: ratatui_hypertile::Towards::Start,
        });
        app.input_mode = InputMode::Normal;
    }
}

fn handle_branch_next(app: &mut App) {
    if app.conversation.focused_block.is_some() {
        app.branch_next();
    } else {
        app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusDirection {
            direction: ratatui::layout::Direction::Horizontal,
            towards: ratatui_hypertile::Towards::End,
        });
        app.input_mode = InputMode::Normal;
    }
}

fn handle_toggle_panel_focus(app: &mut App) {
    if app.has_panel_focus() {
        app.unfocus_panel();
    } else {
        app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusNext);
        app.input_mode = InputMode::Normal;
    }
}

fn handle_directional_focus(app: &mut App, towards: ratatui_hypertile::Towards) {
    app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusDirection {
        direction: ratatui::layout::Direction::Horizontal,
        towards,
    });
    app.input_mode = InputMode::Normal;
}

fn handle_panel_scroll(app: &mut App, up: bool) {
    use clankers_tui_types::PanelId;

    use crate::tui::components::subagent_panel::SubagentPanel;
    if let Some(sp) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
        if up {
            sp.scroll.scroll_up(3);
        } else {
            sp.scroll.scroll_down(3);
        }
    }
}

fn handle_panel_clear_done(app: &mut App) {
    use clankers_tui_types::PanelId;

    use crate::tui::components::subagent_panel::SubagentPanel;
    if let Some(subagent_panel) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
        subagent_panel.clear_done();
        if !subagent_panel.is_visible() {
            app.unfocus_panel();
        }
    }
}

fn handle_panel_kill(
    app: &mut App,
    panel_tx: &tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
) {
    use clankers_tui_types::PanelId;

    use crate::tui::components::subagent_panel::SubagentPanel;
    if let Some(sp) = app.panels.downcast_ref::<SubagentPanel>(PanelId::Subagents)
        && let Some(id) = sp.selected_id()
    {
        let _ = panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest { id });
    }
}

fn handle_panel_remove(app: &mut App) {
    use clankers_tui_types::PanelId;

    use crate::tui::components::subagent_panel::SubagentPanel;
    if let Some(sp) = app.panels.downcast_mut::<SubagentPanel>(PanelId::Subagents) {
        sp.remove_selected();
    }
}

fn handle_toggle_session_popup(app: &mut App) {
    app.overlays.session_popup_visible = !app.overlays.session_popup_visible;
    if app.overlays.session_popup_visible && app.conversation.focused_block.is_none() {
        let last_id = app.conversation.blocks.iter().rev().find_map(|e| match e {
            BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        });
        app.conversation.focused_block = last_id;
    }
}

fn handle_toggle_branch_panel(app: &mut App) {
    use clankers_tui_types::PanelId;

    use crate::tui::components::branch_panel::BranchPanel;

    if app.layout.focused_panel == Some(PanelId::Branches) {
        app.unfocus_panel();
    } else {
        let active_ids = collect_active_block_ids(app);
        if let Some(bp) = app.panels.downcast_mut::<BranchPanel>(PanelId::Branches) {
            bp.refresh(&app.conversation.all_blocks.clone(), &active_ids);
        }
        app.focus_panel(PanelId::Branches);
    }
}

fn handle_open_model_selector(app: &mut App) {
    let models = app.available_models.clone();
    if models.is_empty() {
        app.push_system("No models available.".to_string(), true);
    } else {
        app.overlays.model_selector = crate::tui::components::model_selector::ModelSelector::new(models);
        app.overlays.model_selector.open();
    }
}

fn handle_open_account_selector(app: &mut App) {
    use crate::provider::auth::AuthStoreExt;
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

fn move_focused_pane(app: &mut App, direction: ratatui::layout::Direction, towards: ratatui_hypertile::Towards) {
    app.apply_tiling_action(ratatui_hypertile::HypertileAction::MoveFocused {
        direction,
        towards,
        scope: ratatui_hypertile::MoveScope::Window,
    });
}

fn collect_active_block_ids(app: &App) -> std::collections::HashSet<usize> {
    app.conversation
        .blocks
        .iter()
        .filter_map(|e| match e {
            BlockEntry::Conversation(b) => Some(b.id),
            _ => None,
        })
        .collect()
}
