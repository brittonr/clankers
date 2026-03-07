//! Event loop runner — decomposes the TUI event loop into focused methods.
//!
//! The `EventLoopRunner` struct owns the per-loop state (audit tracking,
//! channels, receivers) and exposes one method per concern:
//! - `drain_agent_events` — agent events, audit logging, session persistence
//! - `drain_panel_events` — subagent panel routing
//! - `drain_todo_requests` — todo tool request/response
//! - `drain_bash_confirms` — bash confirmation prompts
//! - `refresh_peers` — periodic peer registry refresh
//! - `handle_task_results` — prompt completion, login, account switching
//! - `handle_terminal_events` — key dispatch, mouse, paste, overlays

use std::io;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use crate::agent::events::AgentEvent;
use crate::config::keybindings::{Action, InputMode, Keymap};
use crate::error::Result;
use crate::tui::app::App;
use crate::tui::event::AppEvent;
use crate::tui::event as tui_event;
use crate::tui::render;

use super::interactive::{AgentCommand, TaskResult};

/// Owns the per-loop state and channels for the TUI event loop.
pub(crate) struct EventLoopRunner<'a> {
    // Terminal
    terminal: &'a mut Terminal<CrosstermBackend<io::Stdout>>,
    // App state
    app: &'a mut App,
    // Channels
    cmd_tx: tokio::sync::mpsc::UnboundedSender<AgentCommand>,
    done_rx: tokio::sync::mpsc::UnboundedReceiver<TaskResult>,
    event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
    panel_rx: &'a mut tokio::sync::mpsc::UnboundedReceiver<crate::tui::components::subagent_event::SubagentEvent>,
    todo_rx: &'a mut tokio::sync::mpsc::UnboundedReceiver<(
        crate::tools::todo::TodoAction,
        tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
    )>,
    bash_confirm_rx: &'a mut crate::tools::bash::ConfirmRx,
    panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
    // Config
    keymap: Keymap,
    plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
    session_manager: Option<crate::session::SessionManager>,
    db: Option<crate::db::Db>,
    settings: &'a crate::config::settings::Settings,
    // Audit state
    audit_pending: std::collections::HashMap<String, (String, serde_json::Value, std::time::Instant)>,
    audit_seq: u32,
}

impl<'a> EventLoopRunner<'a> {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        terminal: &'a mut Terminal<CrosstermBackend<io::Stdout>>,
        app: &'a mut App,
        event_rx: tokio::sync::broadcast::Receiver<AgentEvent>,
        panel_rx: &'a mut tokio::sync::mpsc::UnboundedReceiver<crate::tui::components::subagent_event::SubagentEvent>,
        todo_rx: &'a mut tokio::sync::mpsc::UnboundedReceiver<(
            crate::tools::todo::TodoAction,
            tokio::sync::oneshot::Sender<crate::tools::todo::TodoResponse>,
        )>,
        bash_confirm_rx: &'a mut crate::tools::bash::ConfirmRx,
        panel_tx: tokio::sync::mpsc::UnboundedSender<crate::tui::components::subagent_event::SubagentEvent>,
        keymap: Keymap,
        plugin_manager: Option<Arc<std::sync::Mutex<crate::plugin::PluginManager>>>,
        session_manager: Option<crate::session::SessionManager>,
        db: Option<crate::db::Db>,
        settings: &'a crate::config::settings::Settings,
        cmd_tx: tokio::sync::mpsc::UnboundedSender<AgentCommand>,
        done_rx: tokio::sync::mpsc::UnboundedReceiver<TaskResult>,
    ) -> Self {
        Self {
            terminal,
            app,
            cmd_tx,
            done_rx,
            event_rx,
            panel_rx,
            todo_rx,
            bash_confirm_rx,
            panel_tx,
            keymap,
            plugin_manager,
            session_manager,
            db,
            settings,
            audit_pending: std::collections::HashMap::new(),
            audit_seq: 0,
        }
    }

    /// Main event loop. Returns when `app.should_quit` is set.
    pub async fn run(&mut self) -> Result<()> {
        loop {
            // Render
            self.terminal.draw(|frame| render::render(frame, self.app)).map_err(|e| {
                crate::error::Error::Tui {
                    message: format!("Render failed: {}", e),
                }
            })?;

            if self.app.should_quit {
                let _ = self.cmd_tx.send(AgentCommand::Quit);
                break;
            }

            self.drain_agent_events();
            self.drain_panel_events();
            self.drain_todo_requests();
            self.drain_bash_confirms();
            self.refresh_peers();
            self.handle_task_results();
            super::clipboard::poll_clipboard_result(self.app);
            self.handle_terminal_events()?;

            // Check for deferred external editor request
            if self.app.open_editor_requested {
                self.app.open_editor_requested = false;
                super::clipboard::open_external_editor(self.terminal, self.app);
            }
        }
        Ok(())
    }

    // ── Agent events + audit logging + session persistence ───────────

    fn drain_agent_events(&mut self) {
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => {
                    self.app.handle_agent_event(&event);

                    // Persist messages to session on AgentEnd
                    if let AgentEvent::AgentEnd { ref messages } = event
                        && let Some(ref mut sm) = self.session_manager
                    {
                        super::interactive::persist_messages(sm, messages);
                    }

                    // Record per-turn usage to redb
                    if let AgentEvent::UsageUpdate { ref turn_usage, .. } = event
                        && let Some(ref db) = self.db
                    {
                        let req = crate::db::usage::RequestUsage::from_provider(&self.app.model, turn_usage);
                        db.spawn_write(move |db| {
                            if let Err(e) = db.usage().record(&req) {
                                tracing::warn!("Failed to record usage: {}", e);
                            }
                        });
                    }

                    // Audit: track tool call start
                    if let AgentEvent::ToolCall {
                        ref call_id,
                        ref tool_name,
                        ref input,
                    } = event
                    {
                        self.audit_pending.insert(
                            call_id.clone(),
                            (tool_name.clone(), input.clone(), std::time::Instant::now()),
                        );
                    }

                    // Audit: record completed tool calls
                    if let AgentEvent::ToolExecutionEnd {
                        ref call_id,
                        ref result,
                        is_error,
                    } = event
                        && let Some(ref db) = self.db
                        && !self.app.session_id.is_empty()
                    {
                        let (tool_name, input, started_at) = self
                            .audit_pending
                            .remove(call_id)
                            .unwrap_or_else(|| ("unknown".into(), serde_json::json!({}), std::time::Instant::now()));
                        let duration_ms = started_at.elapsed().as_millis() as u64;

                        let result_preview: String = result
                            .content
                            .iter()
                            .filter_map(|c| match c {
                                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                                _ => None,
                            })
                            .collect::<Vec<_>>()
                            .join("\n")
                            .chars()
                            .take(500)
                            .collect();

                        let sandbox_blocked = if is_error {
                            result_preview.strip_prefix("🔒 ").map(|s| s.to_string())
                        } else {
                            None
                        };

                        let session_id = self.app.session_id.clone();
                        let call_id = call_id.clone();
                        let seq = self.audit_seq;
                        self.audit_seq += 1;

                        db.spawn_write(move |db| {
                            let entry = crate::db::audit::AuditEntry {
                                session_id,
                                seq,
                                tool: tool_name,
                                call_id,
                                input,
                                is_error,
                                result_preview,
                                duration_ms,
                                timestamp: chrono::Utc::now(),
                                sandbox_blocked,
                            };
                            if let Err(e) = db.audit().record(&entry) {
                                tracing::warn!("Failed to record audit entry: {}", e);
                            }
                        });
                    }

                    // Dispatch to plugins
                    if let Some(ref pm) = self.plugin_manager {
                        let result = super::plugin_dispatch::dispatch_event_to_plugins(pm, &event);
                        for (plugin_name, message) in result.messages {
                            self.app.push_system(format!("🔌 {}: {}", plugin_name, message), false);
                        }
                        for action in result.ui_actions {
                            self.app.plugin_ui.apply(action);
                        }
                    }
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("Agent event receiver lagged, skipped {} events", n);
                    continue;
                }
                Err(_) => break,
            }
        }
    }

    // ── Subagent panel events ───────────────────────────────────────

    fn drain_panel_events(&mut self) {
        while let Ok(event) = self.panel_rx.try_recv() {
            use crate::tui::components::subagent_event::SubagentEvent;
            use crate::tui::components::subagent_panel::SubagentPanel;
            let subagent_panel = self
                .app
                .panels
                .downcast_mut::<SubagentPanel>(crate::tui::panel::PanelId::Subagents)
                .expect("subagent panel");
            match event {
                SubagentEvent::Started { id, name, task, pid } => {
                    subagent_panel.add(id.clone(), name.clone(), task.clone(), pid);
                    let max_panes = self.settings.max_subagent_panes;
                    if max_panes > 0 && self.app.layout.subagent_panes.len() < max_panes {
                        let pane_id = self.app.layout.subagent_panes.create(
                            id.clone(),
                            name,
                            task,
                            pid,
                            &mut self.app.layout.tiling,
                        );
                        self.app
                            .layout
                            .pane_registry
                            .register(pane_id, crate::tui::panes::PaneKind::Subagent(id));
                        crate::tui::panes::auto_split_for_subagent(
                            &mut self.app.layout.tiling,
                            &self.app.layout.pane_registry,
                            pane_id,
                        );
                    }
                }
                SubagentEvent::Output { id, line } => {
                    subagent_panel.append_output(&id, &line);
                    self.app.layout.subagent_panes.append_output(&id, &line);
                }
                SubagentEvent::Done { id } => {
                    subagent_panel.mark_done(&id);
                    self.app.layout.subagent_panes.mark_done(&id);
                }
                SubagentEvent::Error { id, .. } => {
                    subagent_panel.mark_error(&id);
                    self.app.layout.subagent_panes.mark_error(&id);
                }
                SubagentEvent::KillRequest { ref id } => {
                    let pid_to_kill = self
                        .app
                        .layout
                        .subagent_panes
                        .get(id)
                        .filter(|s| {
                            s.status == crate::tui::components::subagent_panel::SubagentStatus::Running
                        })
                        .and_then(|s| s.pid)
                        .or_else(|| {
                            subagent_panel
                                .get_by_id(id)
                                .filter(|e| {
                                    e.status
                                        == crate::tui::components::subagent_panel::SubagentStatus::Running
                                })
                                .and_then(|e| e.pid)
                        });

                    if let Some(pid) = pid_to_kill {
                        #[cfg(unix)]
                        {
                            unsafe {
                                libc::kill(-(pid as i32), libc::SIGKILL);
                            }
                        }
                        #[cfg(not(unix))]
                        {
                            let _ = std::process::Command::new("taskkill")
                                .args(&["/PID", &pid.to_string(), "/F"])
                                .spawn();
                        }
                        subagent_panel.mark_error(id);
                        subagent_panel.append_output(id, "⚡ Killed by user");
                        self.app.layout.subagent_panes.mark_error(id);
                        self.app.layout.subagent_panes.append_output(id, "⚡ Killed by user");
                    } else {
                        subagent_panel.append_output(id, "⚠ Cannot kill: no PID tracked");
                        self.app
                            .layout
                            .subagent_panes
                            .append_output(id, "⚠ Cannot kill: no PID tracked");
                    }
                }
                SubagentEvent::InputRequest { .. } => {}
            }
        }
    }

    // ── Todo tool requests ──────────────────────────────────────────

    fn drain_todo_requests(&mut self) {
        while let Ok((action, resp_tx)) = self.todo_rx.try_recv() {
            use crate::tools::todo::{TodoAction, TodoResponse};
            use crate::tui::components::todo_panel::{TodoPanel, TodoStatus};
            let todo_panel = self
                .app
                .panels
                .downcast_mut::<TodoPanel>(crate::tui::panel::PanelId::Todo)
                .expect("todo panel");

            let response = match action {
                TodoAction::Add { text } => {
                    let id = todo_panel.add(text);
                    TodoResponse::Added { id }
                }
                TodoAction::SetStatus { id, status } => {
                    if let Some(s) = TodoStatus::parse(&status) {
                        if todo_panel.set_status(id, s) {
                            TodoResponse::Updated { id }
                        } else {
                            TodoResponse::NotFound
                        }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::SetStatusByText { query, status } => {
                    if let Some(s) = TodoStatus::parse(&status) {
                        if let Some(id) = todo_panel.set_status_by_text(&query, s) {
                            TodoResponse::Updated { id }
                        } else {
                            TodoResponse::NotFound
                        }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::SetNote { id, note } => {
                    if todo_panel.set_note(id, note) {
                        TodoResponse::Updated { id }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::Remove { id } => {
                    if todo_panel.remove(id) {
                        TodoResponse::Updated { id }
                    } else {
                        TodoResponse::NotFound
                    }
                }
                TodoAction::ClearDone => {
                    todo_panel.clear_done();
                    TodoResponse::Cleared
                }
                TodoAction::List => TodoResponse::Listed {
                    summary: todo_panel.summary(),
                },
            };
            let _ = resp_tx.send(response);
        }
    }

    // ── Bash confirmations ──────────────────────────────────────────

    fn drain_bash_confirms(&mut self) {
        while let Ok((message, resp_tx)) = self.bash_confirm_rx.try_recv() {
            self.app.push_system(message, true);
            self.app
                .push_system("Type 'y' to approve or 'n' to block. Approving...".to_string(), false);
            let _ = resp_tx.send(true);
        }
    }

    // ── Periodic peer refresh ───────────────────────────────────────

    fn refresh_peers(&mut self) {
        use crate::tui::components::peers_panel::PeersPanel;
        static PEER_REFRESH_COUNTER: std::sync::atomic::AtomicU32 =
            std::sync::atomic::AtomicU32::new(0);
        let count = PEER_REFRESH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let peers_panel = self
            .app
            .panels
            .downcast_mut::<PeersPanel>(crate::tui::panel::PanelId::Peers)
            .expect("peers panel");
        if count.is_multiple_of(200) && peers_panel.server_running {
            let registry = crate::modes::rpc::peers::PeerRegistry::load(
                &crate::modes::rpc::peers::registry_path(crate::config::ClankersPaths::get()),
            );
            let entries = crate::tui::components::peers_panel::entries_from_registry(
                &registry,
                chrono::Duration::minutes(5),
            );
            peers_panel.set_peers(entries);
        }
    }

    // ── Task completion handling ────────────────────────────────────

    fn handle_task_results(&mut self) {
        while let Ok(result) = self.done_rx.try_recv() {
            match result {
                TaskResult::PromptDone(Some(e)) => {
                    if let Some(ref mut block) = self.app.conversation.active_block {
                        block.error = Some(format!("{}", e));
                    }
                    self.app.finalize_active_block();
                    if self.app.queued_prompt.is_none() {
                        self.app.push_system(format!("Error: {}", e), true);
                    }
                    if let Some(text) = self.app.queued_prompt.take() {
                        super::event_loop::handle_input_with_plugins(
                            self.app,
                            &text,
                            &self.cmd_tx,
                            self.plugin_manager.as_ref(),
                            &self.panel_tx,
                            &self.db,
                            &mut self.session_manager,
                        );
                    }
                }
                TaskResult::PromptDone(None) => {
                    if let Some(text) = self.app.queued_prompt.take() {
                        super::event_loop::handle_input_with_plugins(
                            self.app,
                            &text,
                            &self.cmd_tx,
                            self.plugin_manager.as_ref(),
                            &self.panel_tx,
                            &self.db,
                            &mut self.session_manager,
                        );
                    }
                }
                TaskResult::LoginDone(Ok(msg)) => self.app.push_system(msg, false),
                TaskResult::LoginDone(Err(msg)) => self.app.push_system(msg, true),
                TaskResult::ThinkingToggled(msg, level) => {
                    self.app.thinking_enabled = level.is_enabled();
                    self.app.thinking_level = level;
                    self.app.push_system(msg, false);
                }

                TaskResult::AccountSwitched(Ok(name)) => {
                    self.app.active_account = name.clone();
                    self.app.push_system(
                        format!(
                            "Switched to account '{}'. New credentials will be used for the next API call.",
                            name
                        ),
                        false,
                    );
                }
                TaskResult::AccountSwitched(Err(msg)) => {
                    self.app.push_system(msg, true);
                }
            }
        }
    }

    // ── Terminal event polling + key dispatch ────────────────────────

    fn handle_terminal_events(&mut self) -> Result<()> {
        let mut poll_timeout = Duration::from_millis(50);
        while let Some(event) = tui_event::poll_event(poll_timeout) {
            poll_timeout = Duration::ZERO;
            match event {
                AppEvent::Paste(text) => {
                    self.app.input_mode = InputMode::Insert;
                    self.app.selection = None;
                    self.app.editor.insert_str(&text);
                    self.app.update_slash_menu();
                }
                AppEvent::Key(key) => {
                    self.handle_key_event(key);
                }
                AppEvent::MouseDown(button, col, row) => {
                    super::mouse::handle_mouse_down(self.app, button, col, row);
                }
                AppEvent::MouseDrag(button, col, row) => {
                    super::mouse::handle_mouse_drag(self.app, button, col, row);
                }
                AppEvent::MouseUp(button, col, row) => {
                    super::mouse::handle_mouse_up(self.app, button, col, row);
                }
                AppEvent::ScrollUp(col, row, n) => {
                    super::mouse::handle_mouse_scroll(self.app, col, row, true, n);
                }
                AppEvent::ScrollDown(col, row, n) => {
                    super::mouse::handle_mouse_scroll(self.app, col, row, false, n);
                }
                AppEvent::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }

    // ── Key event dispatch ──────────────────────────────────────────

    fn handle_key_event(&mut self, key: crossterm::event::KeyEvent) {
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
            && super::event_loop::handle_session_popup_key(self.app, &key, &self.keymap)
        {
            return;
        }
        if self.app.overlays.model_selector.visible
            && super::selectors::handle_model_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.overlays.account_selector.visible
            && super::selectors::handle_account_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.overlays.session_selector.visible
            && super::selectors::handle_session_selector_key(self.app, &key, &self.cmd_tx)
        {
            return;
        }
        if self.app.branching.switcher.visible
            && super::selectors::handle_branch_switcher_key(self.app, &key)
        {
            return;
        }
        if self.app.branching.compare.visible
            && super::selectors::handle_branch_compare_key(self.app, &key)
        {
            return;
        }

        // Merge interactive intercept
        if self.app.branching.merge_interactive.visible
            && super::selectors::handle_merge_interactive_key(self.app, &key)
        {
            if self.app.branching.merge_interactive.confirmed {
                self.handle_merge_confirmed();
            }
            return;
        }

        // Leader menu
        if self.app.overlays.leader_menu.visible {
            if let Some(leader_action) = self.app.overlays.leader_menu.handle_key(&key) {
                super::event_loop::handle_leader_action(
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
            super::event_loop::handle_output_search_key(self.app, &key);
            return;
        }

        // Slash menu (insert mode only)
        if self.app.input_mode == InputMode::Insert
            && self.app.slash_menu.visible
            && super::event_loop::handle_slash_menu_key(
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
                super::clipboard::open_external_editor(self.terminal, self.app);
                return;
            }

            super::event_loop::handle_action(
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
            super::event_loop::handle_insert_char(self.app, &key);
        }
    }

    // ── Panel-focused key handling ──────────────────────────────────

    fn handle_panel_focused_key(&mut self, key: crossterm::event::KeyEvent) -> bool {
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
            let result = self.app.panel_mut(focused_id).handle_key_event(key);
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
                        use crate::tui::components::subagent_panel::SubagentPanel;
                        if let Some(sp) = self.app.panels.downcast_mut::<SubagentPanel>(
                            crate::tui::panel::PanelId::Subagents,
                        ) {
                            sp.open_detail();
                        }
                    }
                    return true;
                }
                None => {}
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

    fn handle_panel_side_effects(&mut self, key: crossterm::event::KeyEvent) -> bool {
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
                use crate::tui::components::peers_panel::PeersPanel;
                let peers_panel = self
                    .app
                    .panels
                    .downcast_mut::<PeersPanel>(PanelId::Peers)
                    .expect("peers panel");
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
                        super::peers_background::probe_peer_background(
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

    fn handle_merge_confirmed(&mut self) {
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
