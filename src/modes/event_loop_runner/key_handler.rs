//! Key event handling for the event loop.
//!
//! This module contains all keyboard input handling logic, extracted from
//! the main event loop runner for better organization.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::config::keybindings::{Action, InputMode};
use crate::modes::{clipboard, event_loop, interactive::AgentCommand, peers_background, selectors};

use super::EventLoopRunner;

impl<'a> EventLoopRunner<'a> {
    // ── Key event dispatch ──────────────────────────────────────────

    pub(super) fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
        self.app.selection = None;

        // Force quit (Ctrl+Q)
        if key.code == KeyCode::Char('q') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.app.should_quit = true;
            return;
        }

        // Overlay intercepts
        if self.app.overlays.cost_overlay_visible
            && matches!(key.code, KeyCode::Esc | KeyCode::Char('C' | 'c' | 'q'))
        {
            self.app.overlays.cost_overlay_visible = false;
            return;
        }

        if self.app.overlays.session_popup_visible
            && event_loop::handle_session_popup_key(self.app, &key, &self.keymap)
        {
            return;
        }
        if self.app.overlays.model_selector.visible
            && selectors::handle_model_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.overlays.account_selector.visible
            && selectors::handle_account_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.overlays.session_selector.visible
            && selectors::handle_session_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.branching.switcher.visible
            && selectors::handle_branch_switcher_key(self.app, &key)
        {
            return;
        }
        if self.app.branching.compare.visible
            && selectors::handle_branch_compare_key(self.app, &key)
        {
            return;
        }

        // Merge interactive intercept
        if self.app.branching.merge_interactive.visible
            && selectors::handle_merge_interactive_key(self.app, &key)
        {
            if self.app.branching.merge_interactive.confirmed {
                self.handle_merge_confirmed();
            }
            return;
        }

        // Leader menu
        if self.app.overlays.leader_menu.visible {
            if let Some(leader_action) = self.app.overlays.leader_menu.handle_key(&key) {
                event_loop::handle_leader_action(
                    self.app,
                    leader_action,
                    &self.cmd_tx,
                    self.plugin_manager.as_ref(),
                    &self.panel_tx,
                    &self.db,
                    &mut self.session_manager,
                );
            }
            return;
        }

        // Output search
        if self.app.overlays.output_search.active {
            event_loop::handle_output_search_key(self.app, &key);
            return;
        }

        // Slash menu (insert mode only)
        if self.app.input_mode == InputMode::Insert
            && self.app.slash_menu.visible
            && event_loop::handle_slash_menu_key(
                self.app,
                &key,
                &self.keymap,
                &self.cmd_tx,
                self.plugin_manager.as_ref(),
                &self.panel_tx,
                &self.db,
                &mut self.session_manager,
            )
        {
            return;
        }

        // Panel intercepts in normal mode
        if self.app.has_panel_focus() && self.app.input_mode == InputMode::Normal && self.handle_panel_focused_key(key) {
            return;
        }

        // Resolve through keymap
        let action = self.keymap.resolve(self.app.input_mode, &key);
        if let Some(action) = action {
            if matches!(&action, Action::Extended(crate::config::keybindings::ExtendedAction::OpenEditor)) {
                clipboard::open_external_editor(self.terminal, self.app);
                return;
            }

            event_loop::handle_action(
                self.app,
                action,
                &key,
                &self.cmd_tx,
                self.plugin_manager.as_ref(),
                &self.panel_tx,
                &self.db,
                &mut self.session_manager,
            );

            // Record branch in session if one was initiated
            if let Some(checkpoint) = self.app.branching.last_branch_checkpoint.take()
                && let Some(ref mut sm) = self.session_manager
                && let Ok(tree) = sm.load_tree()
            {
                let active_leaf = sm.active_leaf_id().cloned();
                let branch_msgs =
                    crate::session::context::build_messages_for_branch(&tree, active_leaf.as_ref());
                if checkpoint > 0 && checkpoint <= branch_msgs.len() {
                    let fork_msg_id = branch_msgs[checkpoint - 1].id().clone();
                    let _ = sm.record_branch(fork_msg_id, "User edited prompt");
                }
            }
        } else if self.app.input_mode == InputMode::Insert {
            event_loop::handle_insert_char(self.app, &key);
        }
    }

    // ── Panel-focused key handling ──────────────────────────────────

    pub(super) fn handle_panel_focused_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crate::tui::panel::PanelAction;

        // Tab / Shift+Tab cycles focus
        if matches!(key.code, KeyCode::Tab) {
            self.app
                .apply_tiling_action(ratatui_hypertile::HypertileAction::FocusNext);
            return true;
        }
        if matches!(key.code, KeyCode::BackTab) {
            self.app
                .apply_tiling_action(ratatui_hypertile::HypertileAction::FocusPrev);
            return true;
        }

        // Tiling keys
        if self.handle_tiling_key(key) {
            return true;
        }

        // Focused tool output
        if self.handle_focused_tool_key(key) {
            return true;
        }

        // Subagent pane keys
        if self.handle_subagent_pane_key(key) {
            return true;
        }

        // Panel side-effect keys
        if self.handle_panel_side_effects(key) {
            return true;
        }

        // Delegate to focused panel's handle_key_event
        if let Some(focused_id) = self.app.layout.focused_panel {
            if let Some(panel) = self.app.panel_mut(focused_id) {
                let result = panel.handle_key_event(key);
                match result {
                    Some(PanelAction::Consumed) => return true,
                    Some(PanelAction::Unfocus) => {
                        self.app.unfocus_panel();
                        return true;
                    }
                    Some(PanelAction::SlashCommand(_cmd)) => return true,
                    Some(PanelAction::SwitchBranch(block_id)) => {
                        self.app.switch_to_branch(block_id);
                        self.app
                            .push_system(format!("Switched to branch at block #{}", block_id), false);
                        return true;
                    }
                    Some(PanelAction::FocusPanel(id)) => {
                        self.app.focus_panel(id);
                        return true;
                    }
                    Some(PanelAction::FocusSubagent(ref subagent_id)) => {
                        if self
                            .app
                            .layout
                            .subagent_panes
                            .pane_id_for(subagent_id)
                            .is_some()
                        {
                            self.app.focus_subagent(subagent_id);
                        } else {
                            super::subagent_panel(self.app).open_detail();
                        }
                        return true;
                    }
                    None => {}
                }
            } else {
                // Panel not registered - unfocus and log error
                tracing::error!(panel_id = ?focused_id, "focused panel not found, unfocusing");
                self.app.unfocus_panel();
                return true;
            }
        }

        false
    }

    fn handle_tiling_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use ratatui::layout::Direction;
        use ratatui_hypertile::{HypertileAction, MoveScope, Towards};

        match (key.code, key.modifiers) {
            (KeyCode::Char('['), m) if m.is_empty() => {
                self.app
                    .apply_tiling_action(HypertileAction::ResizeFocused { delta: -0.05 });
                true
            }
            (KeyCode::Char(']'), m) if m.is_empty() => {
                self.app
                    .apply_tiling_action(HypertileAction::ResizeFocused { delta: 0.05 });
                true
            }
            (KeyCode::Char('H'), m) if m == KeyModifiers::SHIFT => {
                self.app.apply_tiling_action(HypertileAction::MoveFocused {
                    direction: Direction::Horizontal,
                    towards: Towards::Start,
                    scope: MoveScope::Window,
                });
                true
            }
            (KeyCode::Char('L'), m) if m == KeyModifiers::SHIFT => {
                self.app.apply_tiling_action(HypertileAction::MoveFocused {
                    direction: Direction::Horizontal,
                    towards: Towards::End,
                    scope: MoveScope::Window,
                });
                true
            }
            (KeyCode::Char('J'), m) if m == KeyModifiers::SHIFT => {
                self.app.apply_tiling_action(HypertileAction::MoveFocused {
                    direction: Direction::Vertical,
                    towards: Towards::End,
                    scope: MoveScope::Window,
                });
                true
            }
            (KeyCode::Char('K'), m) if m == KeyModifiers::SHIFT => {
                self.app.apply_tiling_action(HypertileAction::MoveFocused {
                    direction: Direction::Vertical,
                    towards: Towards::Start,
                    scope: MoveScope::Window,
                });
                true
            }
            (KeyCode::Char('|'), _) => {
                self.app.split_focused_pane(Direction::Horizontal);
                true
            }
            (KeyCode::Char('-'), m) if m.is_empty() => {
                self.app.split_focused_pane(Direction::Vertical);
                true
            }
            (KeyCode::Char('X'), m) if m == KeyModifiers::SHIFT => {
                self.app.close_focused_pane();
                true
            }
            (KeyCode::Char('='), m) if m.is_empty() => {
                self.app
                    .apply_tiling_action(HypertileAction::SetFocusedRatio { ratio: 0.5 });
                true
            }
            (KeyCode::Char('z'), m) if m.is_empty() => {
                self.app.zoom_toggle();
                true
            }
            _ => false,
        }
    }

    fn handle_focused_tool_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        const TOOL_OUTPUT_VISIBLE: usize = 32;
        let Some(ref call_id) = self.app.streaming.focused_tool.clone() else {
            return false;
        };
        match (key.code, key.modifiers) {
            (KeyCode::Char('j') | KeyCode::Down, m) if m.is_empty() => {
                if let Some(out) = self.app.streaming.outputs.get_mut(call_id) {
                    out.scroll_down(1, TOOL_OUTPUT_VISIBLE);
                }
                true
            }
            (KeyCode::Char('k') | KeyCode::Up, m) if m.is_empty() => {
                if let Some(out) = self.app.streaming.outputs.get_mut(call_id) {
                    out.scroll_up(1);
                }
                true
            }
            (KeyCode::Char('g'), m) if m.is_empty() => {
                if let Some(out) = self.app.streaming.outputs.get_mut(call_id) {
                    out.scroll_to_top();
                }
                true
            }
            (KeyCode::Char('G'), m) if m.is_empty() || m.contains(KeyModifiers::SHIFT) => {
                if let Some(out) = self.app.streaming.outputs.get_mut(call_id) {
                    out.scroll_to_bottom();
                }
                true
            }
            (KeyCode::Char('f'), m) if m.is_empty() => {
                if let Some(out) = self.app.streaming.outputs.get_mut(call_id) {
                    out.toggle_auto_follow();
                }
                true
            }
            (KeyCode::Char('q') | KeyCode::Esc, m) if m.is_empty() => {
                self.app.unfocus_tool();
                true
            }
            _ => false,
        }
    }

    fn handle_subagent_pane_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use crate::tui::panel::PanelAction;
        let Some(ref subagent_id) = self.app.layout.focused_subagent.clone() else {
            return false;
        };
        match (key.code, key.modifiers) {
            (KeyCode::Char('x'), m) if m.is_empty() => {
                let _ = self.panel_tx.send(
                    crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                        id: subagent_id.clone(),
                    },
                );
                true
            }
            (KeyCode::Char('q'), m) if m.is_empty() => {
                if let Some(pane_id) = self.app.layout.subagent_panes.remove(subagent_id) {
                    if let Some(new_root) = crate::tui::panes::remove_pane_from_tree(
                        self.app.layout.tiling.root().clone(),
                        pane_id,
                    ) {
                        let _ = self.app.layout.tiling.set_root(new_root);
                    }
                    self.app.layout.pane_registry.unregister(pane_id);
                    let live: std::collections::HashSet<_> =
                        ratatui_hypertile::raw::collect_pane_ids(self.app.layout.tiling.root())
                            .into_iter()
                            .collect();
                    self.app.layout.pane_registry.retain_only(&live);
                    self.app.sync_focused_panel();
                }
                true
            }
            _ => {
                if let Some(action) = self
                    .app
                    .layout
                    .subagent_panes
                    .handle_key_event(subagent_id, key)
                {
                    match action {
                        PanelAction::Consumed => return true,
                        PanelAction::Unfocus => {
                            self.app.unfocus_panel();
                            return true;
                        }
                        _ => {}
                    }
                }
                false
            }
        }
    }

    pub(super) fn handle_panel_side_effects(&mut self, key: crossterm::event::KeyEvent) -> bool {
        let Some(focused_id) = self.app.layout.focused_panel else {
            return false;
        };
        use crate::tui::panel::PanelId;
        match (focused_id, key.code, key.modifiers) {
            (PanelId::Subagents, KeyCode::Char('x'), m) if m.is_empty() => {
                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(id) = self
                    .app
                    .panels
                    .downcast_ref::<SubagentPanel>(PanelId::Subagents)
                    .expect("subagent panel")
                    .selected_id()
                {
                    let _ = self.panel_tx.send(
                        crate::tui::components::subagent_event::SubagentEvent::KillRequest { id },
                    );
                }
                true
            }
            (PanelId::Peers, KeyCode::Char('p'), m) if m.is_empty() => {
                let peers_panel = super::peers_panel(self.app);
                if let Some(peer) = peers_panel.selected_peer().cloned() {
                    peers_panel.update_status(
                        &peer.node_id,
                        crate::tui::components::peers_panel::PeerStatus::Probing,
                    );
                    let node_id = peer.node_id.clone();
                    let paths = crate::config::ClankersPaths::get();
                    let registry_path = crate::modes::rpc::peers::registry_path(paths);
                    let identity_path = crate::modes::rpc::iroh::identity_path(paths);
                    let ptx = self.panel_tx.clone();
                    tokio::spawn(async move {
                        peers_background::probe_peer_background(
                            node_id,
                            registry_path,
                            identity_path,
                            ptx,
                        )
                        .await;
                    });
                }
                true
            }
            _ => false,
        }
    }

    pub(super) fn handle_merge_confirmed(&mut self) {
        let selected = self.app.branching.merge_interactive.selected_ids();
        let source = self.app.branching.merge_interactive.source_leaf().cloned();
        let target = self.app.branching.merge_interactive.target_leaf().cloned();
        self.app.branching.merge_interactive.close();
        if let (Some(src), Some(tgt)) = (source, target)
            && let Some(sm) = self.session_manager.as_mut()
        {
            match sm.merge_selective(src, tgt, &selected) {
                Ok((count, _new_leaf)) => {
                    if let Ok(context) = sm.build_context() {
                        let msg_count = context.len();
                        let _ = self.cmd_tx.send(AgentCommand::ClearHistory);
                        let _ = self.cmd_tx.send(AgentCommand::SeedMessages(context));
                        self.app.push_system(
                            format!(
                                "Merged {} messages (selective, {} in context)",
                                count, msg_count
                            ),
                            false,
                        );
                    }
                }
                Err(e) => self.app.push_system(format!("Merge failed: {}", e), true),
            }
        }
    }
}
