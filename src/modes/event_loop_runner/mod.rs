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

use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;

use super::interactive::AgentCommand;
use super::interactive::TaskResult;
use crate::agent::events::AgentEvent;
use crate::config::keybindings::InputMode;
use crate::config::keybindings::Keymap;
use crate::error::Result;
use crate::tui::app::App;
use crate::tui::event as tui_event;
use crate::tui::event::AppEvent;
use crate::tui::render;

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
            self.terminal.draw(|frame| render::render(frame, self.app)).map_err(|e| crate::error::Error::Tui {
                message: format!("Render failed: {}", e),
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
                        self.audit_pending
                            .insert(call_id.clone(), (tool_name.clone(), input.clone(), std::time::Instant::now()));
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
            match event {
                SubagentEvent::Started { id, name, task, pid } => {
                    subagent_panel(self.app).add(id.clone(), name.clone(), task.clone(), pid);
                    let max_panes = self.settings.max_subagent_panes;
                    if max_panes > 0 && self.app.layout.subagent_panes.len() < max_panes {
                        let pane_id = self.app.layout.subagent_panes.create(
                            id.clone(),
                            name,
                            task,
                            pid,
                            &mut self.app.layout.tiling,
                        );
                        self.app.layout.pane_registry.register(pane_id, crate::tui::panes::PaneKind::Subagent(id));
                        crate::tui::panes::auto_split_for_subagent(
                            &mut self.app.layout.tiling,
                            &self.app.layout.pane_registry,
                            pane_id,
                        );
                    }
                }
                SubagentEvent::Output { id, line } => {
                    subagent_panel(self.app).append_output(&id, &line);
                    self.app.layout.subagent_panes.append_output(&id, &line);
                }
                SubagentEvent::Done { id } => {
                    subagent_panel(self.app).mark_done(&id);
                    self.app.layout.subagent_panes.mark_done(&id);
                }
                SubagentEvent::Error { id, .. } => {
                    subagent_panel(self.app).mark_error(&id);
                    self.app.layout.subagent_panes.mark_error(&id);
                }
                SubagentEvent::KillRequest { ref id } => {
                    let pid_to_kill = self
                        .app
                        .layout
                        .subagent_panes
                        .get(id)
                        .filter(|s| s.status == crate::tui::components::subagent_panel::SubagentStatus::Running)
                        .and_then(|s| s.pid)
                        .or_else(|| {
                            subagent_panel(self.app)
                                .get_by_id(id)
                                .filter(|e| e.status == crate::tui::components::subagent_panel::SubagentStatus::Running)
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
                            let _ =
                                std::process::Command::new("taskkill").args(&["/PID", &pid.to_string(), "/F"]).spawn();
                        }
                        subagent_panel(self.app).mark_error(id);
                        subagent_panel(self.app).append_output(id, "⚡ Killed by user");
                        self.app.layout.subagent_panes.mark_error(id);
                        self.app.layout.subagent_panes.append_output(id, "⚡ Killed by user");
                    } else {
                        subagent_panel(self.app).append_output(id, "⚠ Cannot kill: no PID tracked");
                        self.app.layout.subagent_panes.append_output(id, "⚠ Cannot kill: no PID tracked");
                    }
                }
                SubagentEvent::InputRequest { .. } => {}
            }
        }
    }

    // ── Todo tool requests ──────────────────────────────────────────

    fn drain_todo_requests(&mut self) {
        while let Ok((action, resp_tx)) = self.todo_rx.try_recv() {
            use crate::tools::todo::TodoAction;
            use crate::tools::todo::TodoResponse;
            use crate::tui::components::todo_panel::TodoStatus;
            let todo_panel = todo_panel(self.app);

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
            self.app.push_system("Type 'y' to approve or 'n' to block. Approving...".to_string(), false);
            let _ = resp_tx.send(true);
        }
    }

    // ── Periodic peer refresh ───────────────────────────────────────

    fn refresh_peers(&mut self) {
        static PEER_REFRESH_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = PEER_REFRESH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let peers_panel = peers_panel(self.app);
        if count.is_multiple_of(200) && peers_panel.server_running {
            let registry = crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(
                crate::config::ClankersPaths::get(),
            ));
            let entries =
                crate::tui::components::peers_panel::entries_from_registry(&registry, chrono::Duration::minutes(5));
            peers_panel.set_peers(entries);
        }
    }

    // ── Task completion handling ────────────────────────────────────

    fn handle_task_results(&mut self) {
        while let Ok(result) = self.done_rx.try_recv() {
            match result {
                TaskResult::PromptDone(Some(e)) => {
                    if let Some(ref mut block) = self.app.conversation.active_block {
                        block.error = Some(e.to_string());
                    }
                    self.app.finalize_active_block();
                    if self.app.queued_prompt.is_none() {
                        self.app.push_system(format!("Error: {}", e), true);
                    }
                    if let Some(text) = self.app.queued_prompt.take() {
                        super::event_handlers::handle_input_with_plugins(
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
                        super::event_handlers::handle_input_with_plugins(
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
                        format!("Switched to account '{}'. New credentials will be used for the next API call.", name),
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
}

// ── Key event handling (extracted to key_handler.rs) ────────────────
mod key_handler;

// ── Panel accessor helpers ──────────────────────────────────────────

/// Helper to access the SubagentPanel. Panics if panel not registered (should never happen).
pub(super) fn subagent_panel(app: &mut App) -> &mut crate::tui::components::subagent_panel::SubagentPanel {
    app.panels
        .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(crate::tui::panel::PanelId::Subagents)
        .expect("subagent panel registered at startup")
}

/// Helper to access the TodoPanel. Panics if panel not registered (should never happen).
pub(super) fn todo_panel(app: &mut App) -> &mut crate::tui::components::todo_panel::TodoPanel {
    app.panels
        .downcast_mut::<crate::tui::components::todo_panel::TodoPanel>(crate::tui::panel::PanelId::Todo)
        .expect("todo panel registered at startup")
}

/// Helper to access the PeersPanel. Panics if panel not registered (should never happen).
pub(super) fn peers_panel(app: &mut App) -> &mut crate::tui::components::peers_panel::PeersPanel {
    app.panels
        .downcast_mut::<crate::tui::components::peers_panel::PeersPanel>(crate::tui::panel::PanelId::Peers)
        .expect("peers panel registered at startup")
}
