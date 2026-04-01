//! Event loop runner — decomposes the TUI event loop into focused methods.
//!
//! The `EventLoopRunner` struct owns the per-loop state (channels, receivers)
//! and delegates backend processing (audit, session persistence, hooks, loop
//! mode, auto-test) to a `SessionController` in embedded mode. It exposes
//! one method per concern:
//! - `drain_agent_events` — real-time TUI rendering + controller event feed
//! - `drain_panel_events` — subagent panel routing
//! - `drain_todo_requests` — todo tool request/response
//! - `drain_bash_confirms` — bash confirmation prompts
//! - `refresh_peers` — periodic peer registry refresh
//! - `handle_task_results` — prompt completion, login, account switching
//! - `handle_terminal_events` — key dispatch, mouse, paste, overlays

use std::io;
use std::sync::Arc;
use std::time::Duration;

use clankers_controller::SessionController;
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

mod key_handler;

/// Owns the per-loop state and channels for the TUI event loop.
///
/// The `SessionController` (in embedded mode) handles audit, session
/// persistence, lifecycle hooks, loop mode, and auto-test. The runner
/// handles TUI rendering, plugin dispatch, usage recording, and user
/// interaction.
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
    db: Option<crate::db::Db>,
    settings: &'a crate::config::settings::Settings,
    // Slash command dispatch
    pub(crate) slash_registry: crate::slash_commands::SlashRegistry,
    // Session controller (handles audit, persistence, hooks, loop, auto-test).
    // Also owns the SessionManager for session persistence and branch/merge
    // operations. Slash commands access it via controller.session_manager.
    controller: SessionController,
    // Schedule event receiver — fired when cron/interval/once schedules trigger.
    schedule_rx: tokio::sync::broadcast::Receiver<clanker_scheduler::ScheduleEvent>,
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
        db: Option<crate::db::Db>,
        settings: &'a crate::config::settings::Settings,
        cmd_tx: tokio::sync::mpsc::UnboundedSender<AgentCommand>,
        done_rx: tokio::sync::mpsc::UnboundedReceiver<TaskResult>,
        slash_registry: crate::slash_commands::SlashRegistry,
        controller: SessionController,
        schedule_rx: tokio::sync::broadcast::Receiver<clanker_scheduler::ScheduleEvent>,
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
            db,
            settings,
            slash_registry,
            controller,
            schedule_rx,
        }
    }

    /// Main event loop. Returns when `app.should_quit` is set.
    #[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; exits on quit signal"))]
    pub fn run(&mut self) -> Result<()> {
        loop {
            self.terminal.draw(|frame| render::render(frame, self.app)).map_err(|e| crate::error::Error::Tui {
                message: format!("Render failed: {}", e),
            })?;

            if self.app.should_quit {
                self.cmd_tx.send(AgentCommand::Quit).ok();
                break;
            }

            self.drain_agent_events();
            self.drain_schedule_events();
            self.drain_panel_events();
            self.drain_todo_requests();
            self.drain_bash_confirms();
            self.refresh_peers();
            self.handle_task_results();
            crate::tui::clipboard::poll_clipboard_result(self.app);
            self.handle_terminal_events()?;

            if self.app.open_editor_requested {
                self.app.open_editor_requested = false;
                crate::tui::clipboard::open_external_editor(self.terminal, self.app);
            }
        }
        Ok(())
    }

    // ── Agent events + TUI rendering + controller feed ──────────────

    #[cfg_attr(dylint_lib = "tigerstyle", allow(unbounded_loop, reason = "event loop; exits on quit signal"))]
    fn drain_agent_events(&mut self) {
        loop {
            match self.event_rx.try_recv() {
                Ok(event) => self.process_agent_event(event),
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("Agent event receiver lagged, skipped {} events", n);
                }
                Err(_) => break,
            }
        }
    }

    /// Process a single agent event: render in TUI, feed to controller,
    /// dispatch to plugins, record usage.
    fn process_agent_event(&mut self, event: AgentEvent) {
        // 1. Translate → TUI (real-time rendering)
        if let Some(tui_event) = crate::event_translator::translate(&event) {
            self.app.handle_tui_event(&tui_event);
        }

        // 2. Feed to controller (audit, hooks, loop tracking, persistence, DaemonEvent)
        self.controller.feed_event(&event);

        // 3. Record usage to redb
        self.record_usage(&event);

        // 4. Dispatch to plugins
        self.dispatch_to_plugins(&event);

        // 5. Persist tool results to redb
        self.persist_tool_result(&event);
    }

    /// Record per-turn usage to redb.
    fn record_usage(&self, event: &AgentEvent) {
        if let AgentEvent::UsageUpdate { turn_usage, .. } = event
            && let Some(ref db) = self.db
        {
            let req = crate::db::usage::RequestUsage::new(
                &self.app.model,
                turn_usage.input_tokens as u64,
                turn_usage.output_tokens as u64,
                turn_usage.cache_creation_input_tokens as u64,
                turn_usage.cache_read_input_tokens as u64,
            );
            db.spawn_write(move |db| {
                if let Err(e) = db.usage().record(&req) {
                    tracing::warn!("Failed to record usage: {}", e);
                }
            });
        }
    }

    /// Persist full tool result content to redb for context compaction.
    fn persist_tool_result(&self, event: &AgentEvent) {
        if let AgentEvent::ToolExecutionEnd {
            call_id,
            result,
            is_error,
        } = event
            && let Some(ref db) = self.db
            && !self.app.session_id.is_empty()
        {
            let content_text: String = result
                .content
                .iter()
                .filter_map(|c| {
                    if let crate::tools::ToolResultContent::Text { text } = c {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let has_image = result.content.iter().any(|c| matches!(c, crate::tools::ToolResultContent::Image { .. }));
            let line_count = content_text.lines().count();
            let byte_count = content_text.len();

            // Tool name comes from controller's DaemonEvent (ToolCall),
            // but we don't have easy access here. Use call_id as fallback.
            let tool_name = String::new();
            let session_id = self.app.session_id.clone();
            let call_id = call_id.clone();
            let is_error = *is_error;
            db.spawn_write(move |db| {
                let entry = crate::db::tool_results::StoredToolResult {
                    session_id,
                    call_id,
                    tool_name,
                    content_text,
                    has_image,
                    is_error,
                    byte_count,
                    line_count,
                };
                if let Err(e) = db.tool_results().store(&entry) {
                    tracing::warn!("Failed to store tool result: {}", e);
                }
            });
        }
    }

    /// Forward events to WASM plugins and apply any UI actions they return.
    fn dispatch_to_plugins(&mut self, event: &AgentEvent) {
        let Some(ref pm) = self.plugin_manager else {
            return;
        };
        let result = super::plugin_dispatch::dispatch_event_to_plugins(pm, event);
        for (plugin_name, message) in result.messages {
            self.app.push_system(format!("🔌 {}: {}", plugin_name, message), false);
        }
        for action in result.ui_actions {
            crate::plugin::ui::apply_ui_action(&mut self.app.plugin_ui, action);
        }
    }

    // ── Schedule events ────────────────────────────────────────────

    /// Drain fired schedule events and inject them as agent prompts.
    ///
    /// When a schedule fires, extracts the `prompt` field from the
    /// payload and sends it to the agent task. Shows a system message
    /// in the TUI so the user knows a schedule triggered.
    fn drain_schedule_events(&mut self) {
        loop {
            match self.schedule_rx.try_recv() {
                Ok(event) => {
                    let prompt = event
                        .payload
                        .get("prompt")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();

                    if prompt.is_empty() {
                        tracing::debug!(
                            "schedule '{}' fired but payload has no 'prompt' field",
                            event.schedule_name,
                        );
                        continue;
                    }

                    self.app.push_system(
                        format!(
                            "⏰ Schedule '{}' fired (#{}) — running prompt",
                            event.schedule_name, event.fire_count,
                        ),
                        false,
                    );
                    self.cmd_tx.send(AgentCommand::Prompt(prompt)).ok();
                }
                Err(tokio::sync::broadcast::error::TryRecvError::Lagged(n)) => {
                    tracing::warn!("Schedule event receiver lagged, skipped {n} events");
                }
                Err(_) => break,
            }
        }
    }

    // ── Subagent panel events ───────────────────────────────────────

    fn drain_panel_events(&mut self) {
        while let Ok(event) = self.panel_rx.try_recv() {
            use clankers_tui_types::SubagentEvent;
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
                    self.handle_kill_request(id);
                }
                SubagentEvent::InputRequest { .. } => {}
            }
        }
    }

    /// Handle a subagent kill request — find the PID and send SIGKILL.
    fn handle_kill_request(&mut self, id: &str) {
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
                std::process::Command::new("taskkill").args(&["/PID", &pid.to_string(), "/F"]).spawn().ok();
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

    // ── Todo tool requests ──────────────────────────────────────────

    fn drain_todo_requests(&mut self) {
        while let Ok((action, resp_tx)) = self.todo_rx.try_recv() {
            let response = process_todo_action(todo_panel(self.app), action);
            resp_tx.send(response).ok();
        }
    }

    // ── Bash confirmations ──────────────────────────────────────────

    fn drain_bash_confirms(&mut self) {
        while let Ok(req) = self.bash_confirm_rx.try_recv() {
            self.app.push_system(
                format!("⚠️  Dangerous command detected ({}): {}", req.reason, req.command),
                true,
            );
            self.app.push_system("Type 'y' to approve or 'n' to block. Approving...".to_string(), false);
            req.resp_tx.send(true).ok();
        }
    }

    // ── Periodic peer refresh ───────────────────────────────────────

    fn refresh_peers(&mut self) {
        static PEER_REFRESH_COUNTER: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let count = PEER_REFRESH_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let peers = peers_panel(self.app);
        if count.is_multiple_of(200) && peers.server_running {
            let registry = crate::modes::rpc::peers::PeerRegistry::load(&crate::modes::rpc::peers::registry_path(
                crate::config::ClankersPaths::get(),
            ));
            let entries = crate::tui::components::peers_panel::entries_from_registry(
                &crate::modes::rpc::peers::peer_info_views(&registry),
                chrono::Duration::minutes(5),
            );
            peers.set_peers(entries);
        }
    }

    // ── Task completion handling (delegates to controller) ──────────

    fn handle_task_results(&mut self) {
        while let Ok(result) = self.done_rx.try_recv() {
            match result {
                TaskResult::PromptDone(Some(e)) => {
                    // Notify controller so it clears busy + finishes loop on error
                    self.controller.notify_prompt_done(true);

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
                            &mut None, // session persistence handled by controller
                            &self.slash_registry,
                        );
                    }
                    // Drain any controller events (e.g. loop finish message)
                    self.drain_controller_messages();
                }
                TaskResult::PromptDone(None) => {
                    // Notify controller that prompt succeeded
                    self.controller.notify_prompt_done(false);

                    if let Some(text) = self.app.queued_prompt.take() {
                        super::event_handlers::handle_input_with_plugins(
                            self.app,
                            &text,
                            &self.cmd_tx,
                            self.plugin_manager.as_ref(),
                            &self.panel_tx,
                            &self.db,
                            &mut None, // session persistence handled by controller
                            &self.slash_registry,
                        );
                    } else {
                        // Sync TUI loop state to controller, then check what to do
                        self.controller.sync_loop_from_tui(self.app.loop_status.as_ref());

                        match self.controller.check_post_prompt() {
                            clankers_controller::PostPromptAction::ContinueLoop(prompt) => {
                                // Sync iteration count back to TUI
                                if let Some(iter) = self.controller.loop_iteration()
                                    && let Some(ref mut ls) = self.app.loop_status
                                {
                                    ls.iteration = iter;
                                }
                                // Paused check — only continue if TUI says active
                                if self.app.loop_status.as_ref().is_some_and(|ls| ls.active) {
                                    self.cmd_tx.send(AgentCommand::ResetCancel).ok();
                                    self.cmd_tx.send(AgentCommand::Prompt(prompt)).ok();
                                }
                            }
                            clankers_controller::PostPromptAction::RunAutoTest(prompt) => {
                                self.app.push_system(
                                    format!("🧪 Running auto-test: {}", self.app.auto_test_command.as_deref().unwrap_or("?")),
                                    false,
                                );
                                self.cmd_tx.send(AgentCommand::ResetCancel).ok();
                                self.cmd_tx.send(AgentCommand::Prompt(prompt)).ok();
                            }
                            clankers_controller::PostPromptAction::None => {}
                        }
                    }
                    // Drain any controller events (e.g. loop finish messages)
                    self.drain_controller_messages();
                }
                TaskResult::LoginDone(Ok(msg)) => self.app.push_system(msg, false),
                TaskResult::LoginDone(Err(msg)) => self.app.push_system(msg, true),
                TaskResult::ThinkingToggled(msg, level) => {
                    self.app.thinking_enabled = level.is_enabled();
                    self.app.thinking_level = level;
                    self.app.push_system(msg, false);
                }
                TaskResult::AccountSwitched(Ok(name)) => {
                    self.app.active_account.clone_from(&name);
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

    /// Process controller outgoing events (system messages from loop/auto-test).
    fn drain_controller_messages(&mut self) {
        for event in self.controller.take_outgoing() {
            match event {
                clankers_protocol::DaemonEvent::SystemMessage { text, is_error } => {
                    self.app.push_system(text, is_error);
                }
                _ => {
                    // Other DaemonEvents (audit, lifecycle) are internal — ignore in TUI
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
                    crate::tui::mouse::handle_mouse_down(self.app, button, col, row);
                }
                AppEvent::MouseDrag(button, col, row) => {
                    crate::tui::mouse::handle_mouse_drag(self.app, button, col, row);
                }
                AppEvent::MouseUp(button, col, row) => {
                    crate::tui::mouse::handle_mouse_up(self.app, button, col, row);
                }
                AppEvent::ScrollUp(col, row, n) => {
                    crate::tui::mouse::handle_mouse_scroll(self.app, col, row, true, n);
                }
                AppEvent::ScrollDown(col, row, n) => {
                    crate::tui::mouse::handle_mouse_scroll(self.app, col, row, false, n);
                }
                AppEvent::Resize(_, _) => {}
                _ => {}
            }
        }
        Ok(())
    }
}

// ── Todo action processor ───────────────────────────────────────────

fn process_todo_action(
    panel: &mut crate::tui::components::todo_panel::TodoPanel,
    action: crate::tools::todo::TodoAction,
) -> crate::tools::todo::TodoResponse {
    use crate::tools::todo::TodoAction;
    use crate::tools::todo::TodoResponse;
    use crate::tui::components::todo_panel::TodoStatus;

    match action {
        TodoAction::Add { text } => {
            let id = panel.add(text);
            TodoResponse::Added { id }
        }
        TodoAction::SetStatus { id, status } => {
            if let Some(s) = TodoStatus::parse(&status) {
                if panel.set_status(id, s) {
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
                if let Some(id) = panel.set_status_by_text(&query, s) {
                    TodoResponse::Updated { id }
                } else {
                    TodoResponse::NotFound
                }
            } else {
                TodoResponse::NotFound
            }
        }
        TodoAction::SetNote { id, note } => {
            if panel.set_note(id, note) {
                TodoResponse::Updated { id }
            } else {
                TodoResponse::NotFound
            }
        }
        TodoAction::Remove { id } => {
            if panel.remove(id) {
                TodoResponse::Updated { id }
            } else {
                TodoResponse::NotFound
            }
        }
        TodoAction::ClearDone => {
            panel.clear_done();
            TodoResponse::Cleared
        }
        TodoAction::List => TodoResponse::Listed {
            summary: panel.summary(),
        },
    }
}

// ── Panel accessor helpers ──────────────────────────────────────────

#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "panel registered at startup"))]
pub(super) fn subagent_panel(app: &mut App) -> &mut crate::tui::components::subagent_panel::SubagentPanel {
    app.panels
        .downcast_mut::<crate::tui::components::subagent_panel::SubagentPanel>(crate::tui::panel::PanelId::Subagents)
        .expect("subagent panel registered at startup")
}

#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "panel registered at startup"))]
pub(super) fn todo_panel(app: &mut App) -> &mut crate::tui::components::todo_panel::TodoPanel {
    app.panels
        .downcast_mut::<crate::tui::components::todo_panel::TodoPanel>(crate::tui::panel::PanelId::Todo)
        .expect("todo panel registered at startup")
}

#[cfg_attr(dylint_lib = "tigerstyle", allow(no_unwrap, reason = "panel registered at startup"))]
pub(super) fn peers_panel(app: &mut App) -> &mut crate::tui::components::peers_panel::PeersPanel {
    app.panels
        .downcast_mut::<crate::tui::components::peers_panel::PeersPanel>(crate::tui::panel::PanelId::Peers)
        .expect("peers panel registered at startup")
}
