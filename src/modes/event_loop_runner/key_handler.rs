//! Key event handling for the event loop.
//!
//! This module contains all keyboard input handling logic, extracted from
//! the main event loop runner for better organization.

use crossterm::event::KeyCode;
use crossterm::event::KeyModifiers;

use super::EventLoopRunner;
use crate::config::keybindings::Action;
use crate::config::keybindings::InputMode;
use crate::modes::event_handlers;
use crate::modes::interactive::AgentCommand;
use crate::modes::peers_background;
use crate::tui::clipboard;
use crate::tui::selectors;

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
        if self.app.overlays.cost_overlay_visible && matches!(key.code, KeyCode::Esc | KeyCode::Char('C' | 'c' | 'q')) {
            self.app.overlays.cost_overlay_visible = false;
            return;
        }

        if self.app.overlays.session_popup_visible
            && event_handlers::handle_session_popup_key(self.app, &key, &self.keymap)
        {
            return;
        }
        if self.app.overlays.model_selector.visible {
            let (consumed, action) = selectors::handle_model_selector_key(self.app, &key);
            if let Some(action) = action {
                self.dispatch_selector_action(action);
            }
            if consumed {
                return;
            }
        }
        if self.app.overlays.account_selector.visible {
            let (consumed, action) = selectors::handle_account_selector_key(self.app, &key);
            if let Some(action) = action {
                self.dispatch_selector_action(action);
            }
            if consumed {
                return;
            }
        }
        if self.app.overlays.session_selector.visible {
            let (consumed, action) = selectors::handle_session_selector_key(self.app, &key);
            if let Some(action) = action {
                self.dispatch_selector_action(action);
            }
            if consumed {
                return;
            }
        }
        if self.app.overlays.tool_toggle.visible {
            let (consumed, dirty) = selectors::handle_tool_toggle_key(self.app, &key);
            if dirty {
                self.apply_tool_toggle();
            }
            if consumed {
                return;
            }
        }
        if self.app.branching.switcher.visible() && selectors::handle_branch_switcher_key(self.app, &key) {
            return;
        }
        if self.app.branching.compare.visible && selectors::handle_branch_compare_key(self.app, &key) {
            return;
        }

        // Merge interactive intercept
        if self.app.branching.merge_interactive.visible && selectors::handle_merge_interactive_key(self.app, &key) {
            if self.app.branching.merge_interactive.confirmed {
                self.handle_merge_confirmed();
            }
            return;
        }

        // Leader menu
        if self.app.overlays.leader_menu.visible {
            if let Some(leader_action) = self.app.overlays.leader_menu.handle_key(&key) {
                event_handlers::handle_leader_action(
                    self.app,
                    leader_action,
                    &self.cmd_tx,
                    self.plugin_manager.as_ref(),
                    &self.panel_tx,
                    &self.db,
                    &mut self.controller.session_manager,
                    &self.slash_registry,
                );
            }
            return;
        }

        // Output search
        if self.app.overlays.output_search.active {
            event_handlers::handle_output_search_key(self.app, &key);
            return;
        }

        // Slash menu (insert mode only)
        if self.app.input_mode == InputMode::Insert
            && self.app.slash_menu.visible
            && event_handlers::handle_slash_menu_key(
                self.app,
                &key,
                &self.keymap,
                &self.cmd_tx,
                self.plugin_manager.as_ref(),
                &self.panel_tx,
                &self.db,
                &mut self.controller.session_manager,
                &self.slash_registry,
            )
        {
            return;
        }

        // Panel intercepts in normal mode
        if self.app.has_panel_focus() && self.app.input_mode == InputMode::Normal && self.handle_panel_focused_key(key)
        {
            return;
        }

        // Resolve through keymap
        let action = self.keymap.resolve(self.app.input_mode, &key);
        if let Some(action) = action {
            if matches!(&action, Action::Extended(crate::config::keybindings::ExtendedAction::OpenEditor)) {
                clipboard::open_external_editor(self.terminal, self.app);
                return;
            }

            event_handlers::handle_action(
                self.app,
                action,
                &key,
                &self.cmd_tx,
                self.plugin_manager.as_ref(),
                &self.panel_tx,
                &self.db,
                &mut self.controller.session_manager,
                &self.slash_registry,
            );

            // Record branch in session if one was initiated
            if let Some(checkpoint) = self.app.branching.last_branch_checkpoint.take()
                && let Some(ref mut sm) = self.controller.session_manager
                && let Ok(tree) = sm.load_tree()
            {
                let active_leaf = sm.active_leaf_id().cloned();
                let branch_msgs = crate::session::context::build_messages_for_branch(&tree, active_leaf.as_ref());
                if checkpoint > 0 && checkpoint <= branch_msgs.len() {
                    let fork_msg_id = branch_msgs[checkpoint - 1].id().clone();
                    let _ = sm.record_branch(fork_msg_id, "User edited prompt");
                }
            }
        } else if self.app.input_mode == InputMode::Insert {
            event_handlers::handle_insert_char(self.app, &key);
        }
    }

    // ── Panel-focused key handling ──────────────────────────────────

    pub(super) fn handle_panel_focused_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
        use clankers_tui_types::PanelAction;

        // Tab / Shift+Tab cycles focus
        if matches!(key.code, KeyCode::Tab) {
            self.app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusNext);
            return true;
        }
        if matches!(key.code, KeyCode::BackTab) {
            self.app.apply_tiling_action(ratatui_hypertile::HypertileAction::FocusPrev);
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
                        self.app.push_system(format!("Switched to branch at block #{}", block_id), false);
                        return true;
                    }
                    Some(PanelAction::FocusPanel(id)) => {
                        self.app.focus_panel(id);
                        return true;
                    }
                    Some(PanelAction::FocusSubagent(ref subagent_id)) => {
                        if self.app.layout.subagent_panes.pane_id_for(subagent_id).is_some() {
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
        use ratatui_hypertile::HypertileAction;
        use ratatui_hypertile::MoveScope;
        use ratatui_hypertile::Towards;

        match (key.code, key.modifiers) {
            (KeyCode::Char('['), m) if m.is_empty() => {
                self.app.apply_tiling_action(HypertileAction::ResizeFocused { delta: -0.05 });
                true
            }
            (KeyCode::Char(']'), m) if m.is_empty() => {
                self.app.apply_tiling_action(HypertileAction::ResizeFocused { delta: 0.05 });
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
                self.app.apply_tiling_action(HypertileAction::SetFocusedRatio { ratio: 0.5 });
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
        use clankers_tui_types::PanelAction;
        let Some(ref subagent_id) = self.app.layout.focused_subagent.clone() else {
            return false;
        };
        match (key.code, key.modifiers) {
            (KeyCode::Char('x'), m) if m.is_empty() => {
                let _ = self.panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest {
                    id: subagent_id.clone(),
                });
                true
            }
            (KeyCode::Char('q'), m) if m.is_empty() => {
                if let Some(pane_id) = self.app.layout.subagent_panes.remove(subagent_id) {
                    if let Some(new_root) =
                        crate::tui::panes::remove_pane_from_tree(self.app.layout.tiling.root().clone(), pane_id)
                    {
                        let _ = self.app.layout.tiling.set_root(new_root);
                    }
                    self.app.layout.pane_registry.unregister(pane_id);
                    let live: std::collections::HashSet<_> =
                        ratatui_hypertile::raw::collect_pane_ids(self.app.layout.tiling.root()).into_iter().collect();
                    self.app.layout.pane_registry.retain_only(&live);
                    self.app.sync_focused_panel();
                }
                true
            }
            _ => {
                if let Some(action) = self.app.layout.subagent_panes.handle_key_event(subagent_id, key) {
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
        use clankers_tui_types::PanelId;
        match (focused_id, key.code, key.modifiers) {
            (PanelId::Subagents, KeyCode::Char('x'), m) if m.is_empty() => {
                use crate::tui::components::subagent_panel::SubagentPanel;
                if let Some(id) = self
                    .app
                    .panels
                    .downcast_ref::<SubagentPanel>(PanelId::Subagents)
                    .expect("subagent panel registered at startup")
                    .selected_id()
                {
                    let _ =
                        self.panel_tx.send(crate::tui::components::subagent_event::SubagentEvent::KillRequest { id });
                }
                true
            }
            (PanelId::Peers, KeyCode::Char('p'), m) if m.is_empty() => {
                let peers_panel = super::peers_panel(self.app);
                if let Some(peer) = peers_panel.selected_peer().cloned() {
                    peers_panel.update_status(&peer.node_id, crate::tui::components::peers_panel::PeerStatus::Probing);
                    let node_id = peer.node_id.clone();
                    let paths = crate::config::ClankersPaths::get();
                    let registry_path = crate::modes::rpc::peers::registry_path(paths);
                    let identity_path = crate::modes::rpc::iroh::identity_path(paths);
                    let ptx = self.panel_tx.clone();
                    tokio::spawn(async move {
                        peers_background::probe_peer_background(node_id, registry_path, identity_path, ptx).await;
                    });
                }
                true
            }
            _ => false,
        }
    }

    /// Map a `SelectorAction` to the appropriate `AgentCommand` or side-effect.
    fn dispatch_selector_action(&mut self, action: clankers_tui_types::SelectorAction) {
        use clankers_tui_types::SelectorAction;
        match action {
            SelectorAction::SetModel(model) => {
                let _ = self.cmd_tx.send(AgentCommand::SetModel(model));
            }
            SelectorAction::SwitchAccount(name) => {
                let _ = self.cmd_tx.send(AgentCommand::SwitchAccount(name));
            }
            SelectorAction::ResumeSession { file_path, session_id } => {
                super::super::interactive::resume_session_from_file(self.app, file_path, &session_id, &self.cmd_tx);
            }
        }
    }

    /// Apply tool toggle changes: update disabled_tools, persist if needed,
    /// and send a rebuild command to the agent.
    pub(super) fn apply_tool_toggle(&mut self) {
        use crate::tui::components::tool_toggle::ToolToggleScope;

        let disabled = self.app.overlays.tool_toggle.disabled_set();
        let scope = self.app.overlays.tool_toggle.scope;

        // Update app state
        self.app.disabled_tools.clone_from(&disabled);

        // Persist based on scope
        match scope {
            ToolToggleScope::Session => {
                // No persistence — disabled_tools only lives in app state
            }
            ToolToggleScope::Project => {
                let project_paths = crate::config::ProjectPaths::resolve(std::path::Path::new(&self.app.cwd));
                Self::persist_disabled_tools(&project_paths.settings, &disabled);
            }
            ToolToggleScope::Global => {
                let paths = crate::config::ClankersPaths::get();
                Self::persist_disabled_tools(&paths.global_settings, &disabled);
            }
        }

        // Rebuild the agent's tool set via command
        let _ = self.cmd_tx.send(AgentCommand::SetDisabledTools(disabled.clone()));

        let enabled_count = self.app.overlays.tool_toggle.entries.iter().filter(|e| e.enabled).count();
        let total = self.app.overlays.tool_toggle.entries.len();
        let disabled_count = total - enabled_count;
        if disabled_count > 0 {
            self.app.push_system(
                format!("Tools updated: {enabled_count} enabled, {disabled_count} disabled (scope: {scope})"),
                false,
            );
        } else {
            self.app.push_system("All tools enabled.".to_string(), false);
        }
    }

    /// Persist disabled_tools to a settings.json file.
    /// Reads existing content, merges the disabledTools field, and writes back.
    fn persist_disabled_tools(path: &std::path::Path, disabled: &std::collections::HashSet<String>) {
        // Read existing
        let mut value: serde_json::Value = if let Ok(content) = std::fs::read_to_string(path) {
            serde_json::from_str(&content).unwrap_or_else(|_| serde_json::json!({}))
        } else {
            serde_json::json!({})
        };

        // Update the field
        let sorted: Vec<&String> = {
            let mut v: Vec<&String> = disabled.iter().collect();
            v.sort();
            v
        };
        if let Some(obj) = value.as_object_mut() {
            if disabled.is_empty() {
                obj.remove("disabledTools");
            } else {
                obj.insert("disabledTools".to_string(), serde_json::json!(sorted));
            }
        }

        // Ensure parent dir exists and write
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(&value) {
            let _ = std::fs::write(path, content);
        }
    }

    pub(super) fn handle_merge_confirmed(&mut self) {
        use crate::provider::message::MessageId;
        let selected: Vec<MessageId> =
            self.app.branching.merge_interactive.selected_ids().into_iter().map(MessageId::from).collect();
        let source: Option<MessageId> =
            self.app.branching.merge_interactive.source_leaf().map(|s| MessageId::from(s.to_owned()));
        let target: Option<MessageId> =
            self.app.branching.merge_interactive.target_leaf().map(|s| MessageId::from(s.to_owned()));
        self.app.branching.merge_interactive.close();
        if let (Some(src), Some(tgt)) = (source, target)
            && let Some(sm) = self.controller.session_manager.as_mut()
        {
            match sm.merge_selective(src, tgt, &selected) {
                Ok((count, _new_leaf)) => {
                    if let Ok(context) = sm.build_context() {
                        let msg_count = context.len();
                        let _ = self.cmd_tx.send(AgentCommand::ClearHistory);
                        let _ = self.cmd_tx.send(AgentCommand::SeedMessages(context));
                        self.app.push_system(
                            format!("Merged {} messages (selective, {} in context)", count, msg_count),
                            false,
                        );
                    }
                }
                Err(e) => self.app.push_system(format!("Merge failed: {}", e), true),
            }
        }
    }
}
