//! Transport-agnostic session controller for agent orchestration.
//!
//! The `SessionController` owns one agent session and accepts `SessionCommand`
//! inputs, emitting `DaemonEvent` outputs. It does not know about terminals,
//! sockets, or rendering — that's the client's job.
//!
//! This is the core piece extracted from `EventLoopRunner` that contains all
//! the non-TUI agent orchestration logic.

pub mod audit;
pub mod capability;
pub mod client;
pub mod config;
pub mod confirm;
pub mod convert;
pub mod loop_mode;
pub mod persistence;
pub mod transport;

use std::collections::HashMap;
use std::sync::Arc;

use clankers_agent::Agent;
use clankers_agent::AgentError;
use clankers_agent::events::AgentEvent;
use clankers_hooks::HookPipeline;
use clankers_loop::LoopEngine;
use clankers_loop::LoopId;
use clankers_protocol::DaemonEvent;
use clankers_protocol::SessionCommand;
use clankers_provider::message::Content;
use clankers_session::SessionManager;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;
use tracing::warn;

use self::audit::AuditTracker;
use self::config::ControllerConfig;
use self::confirm::ConfirmStore;
use self::convert::agent_event_to_daemon_event;

/// Action the TUI should take after a prompt completes.
#[derive(Debug)]
pub enum PostPromptAction {
    /// No continuation — prompt is done.
    None,
    /// Continue a loop iteration with this prompt.
    ContinueLoop(String),
    /// Run an auto-test with this prompt.
    RunAutoTest(String),
}

/// Transport-agnostic orchestrator that owns one agent session.
///
/// Accepts `SessionCommand`, emits `DaemonEvent`. Does not know about
/// terminals, sockets, or rendering.
///
/// Two modes:
/// - **Daemon mode** (`new`): owns the Agent, drives prompts directly.
/// - **Embedded mode** (`new_embedded`): no agent ownership. Events fed
///   via `feed_event()`, agent managed externally by `agent_task`.
#[allow(dead_code)] // Fields used incrementally as phases are implemented
pub struct SessionController {
    /// The agent instance (None in embedded mode).
    agent: Option<Agent>,
    /// Receiver for agent events (None in embedded mode).
    event_rx: Option<broadcast::Receiver<AgentEvent>>,
    /// Session persistence.
    session_manager: Option<SessionManager>,
    /// Loop engine for loop/retry iteration.
    loop_engine: LoopEngine,
    /// Active loop ID.
    active_loop_id: Option<LoopId>,
    /// Accumulated tool output for break conditions.
    loop_turn_output: String,
    /// Lifecycle hooks pipeline.
    hook_pipeline: Option<Arc<HookPipeline>>,
    /// Tool call timing and leak detection.
    audit: AuditTracker,
    /// Maps call_id → tool_name for tool result persistence.
    tool_call_names: HashMap<String, String>,
    /// Pending bash confirmations.
    bash_confirms: ConfirmStore<bool>,
    /// Pending todo responses.
    todo_confirms: ConfirmStore<serde_json::Value>,
    /// Queued outgoing events.
    outgoing: Vec<DaemonEvent>,
    /// Whether a prompt is currently in progress.
    busy: bool,
    /// Prevents recursive auto-test triggers.
    auto_test_in_progress: bool,
    /// Auto-test command from settings.
    auto_test_command: Option<String>,
    /// Whether auto-test is enabled.
    auto_test_enabled: bool,
    /// Session ID.
    session_id: String,
    /// Capability restrictions (None = full access).
    capabilities: Option<Vec<String>>,
    /// Current model name.
    model: String,
}

impl SessionController {
    /// Create a new controller that owns the Agent (daemon mode).
    pub fn new(agent: Agent, config: ControllerConfig) -> Self {
        let event_rx = agent.subscribe();
        let model = config.model.clone();

        Self {
            agent: Some(agent),
            event_rx: Some(event_rx),
            session_manager: config.session_manager,
            loop_engine: LoopEngine::new(),
            active_loop_id: None,
            loop_turn_output: String::new(),
            hook_pipeline: config.hook_pipeline,
            audit: AuditTracker::new(),
            tool_call_names: HashMap::new(),
            bash_confirms: ConfirmStore::new(),
            todo_confirms: ConfirmStore::new(),
            outgoing: Vec::new(),
            busy: false,
            auto_test_in_progress: false,
            auto_test_command: config.auto_test_command,
            auto_test_enabled: config.auto_test_enabled,
            session_id: config.session_id,
            capabilities: config.capabilities,
            model,
        }
    }

    /// Create a controller without an Agent (embedded mode).
    ///
    /// In this mode the agent is managed externally by `agent_task`.
    /// Events are fed via [`feed_event`] instead of draining an internal
    /// broadcast receiver. Use [`take_outgoing`] to collect emitted
    /// `DaemonEvent`s.
    pub fn new_embedded(config: ControllerConfig) -> Self {
        let model = config.model.clone();

        Self {
            agent: None,
            event_rx: None,
            session_manager: config.session_manager,
            loop_engine: LoopEngine::new(),
            active_loop_id: None,
            loop_turn_output: String::new(),
            hook_pipeline: config.hook_pipeline,
            audit: AuditTracker::new(),
            tool_call_names: HashMap::new(),
            bash_confirms: ConfirmStore::new(),
            todo_confirms: ConfirmStore::new(),
            outgoing: Vec::new(),
            busy: false,
            auto_test_in_progress: false,
            auto_test_command: config.auto_test_command,
            auto_test_enabled: config.auto_test_enabled,
            session_id: config.session_id,
            capabilities: config.capabilities,
            model,
        }
    }

    /// Process a command from a client. Requires daemon mode (agent owned).
    ///
    /// In embedded mode, commands are handled by the agent_task;
    /// the controller only processes events via `feed_event()`.
    pub async fn handle_command(&mut self, cmd: SessionCommand) {
        if self.agent.is_none() {
            warn!("handle_command called in embedded mode (no agent)");
            return;
        }

        match cmd {
            SessionCommand::Prompt { text, images } => {
                self.handle_prompt(text, images).await;
            }
            SessionCommand::Abort => {
                if let Some(ref mut agent) = self.agent {
                    agent.abort();
                }
                self.busy = false;
                self.emit(DaemonEvent::SystemMessage {
                    text: "Operation cancelled".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::ResetCancel => {
                if let Some(ref mut agent) = self.agent {
                    agent.reset_cancel();
                }
            }
            SessionCommand::SetModel { model } => {
                let from = self.model.clone();
                if let Some(ref mut agent) = self.agent {
                    agent.set_model(model.clone());
                }
                self.model = model.clone();
                self.emit(DaemonEvent::ModelChanged {
                    from,
                    to: model,
                    reason: "user request".to_string(),
                });
            }
            SessionCommand::ClearHistory => {
                if let Some(ref mut agent) = self.agent {
                    agent.clear_messages();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "History cleared".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::TruncateMessages { count } => {
                if let Some(ref mut agent) = self.agent {
                    agent.truncate_messages(count);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Truncated to {count} messages"),
                    is_error: false,
                });
            }
            SessionCommand::SetThinkingLevel { level } => {
                if let Some(parsed) = clankers_tui_types::ThinkingLevel::from_str_or_budget(&level) {
                    if let Some(ref mut agent) = self.agent {
                        let prev = agent.set_thinking_level(parsed);
                        self.emit(DaemonEvent::SystemMessage {
                            text: format!("Thinking: {} → {}", prev.label(), parsed.label()),
                            is_error: false,
                        });
                    }
                } else {
                    self.emit(DaemonEvent::SystemMessage {
                        text: format!("Unknown thinking level: {level}"),
                        is_error: true,
                    });
                }
            }
            SessionCommand::CycleThinkingLevel => {
                if let Some(ref mut agent) = self.agent {
                    agent.cycle_thinking_level();
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "Thinking level cycled".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::SeedMessages { messages } => {
                let agent_messages = self.convert_seed_messages(&messages);
                let count = agent_messages.len();
                if let Some(ref mut agent) = self.agent {
                    agent.seed_messages(agent_messages);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Seeded {count} messages"),
                    is_error: false,
                });
            }
            SessionCommand::SetSystemPrompt { prompt } => {
                if let Some(ref mut agent) = self.agent {
                    agent.set_system_prompt(prompt);
                }
                self.emit(DaemonEvent::SystemMessage {
                    text: "System prompt updated".to_string(),
                    is_error: false,
                });
            }
            SessionCommand::GetSystemPrompt => {
                let prompt = self
                    .agent
                    .as_ref()
                    .map(|a| a.system_prompt().to_string())
                    .unwrap_or_default();
                self.emit(DaemonEvent::SystemPromptResponse { prompt });
            }
            SessionCommand::SwitchAccount { account } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Account switch to '{account}' requested"),
                    is_error: false,
                });
            }
            SessionCommand::SetDisabledTools { tools } => {
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Disabled tools updated: {}", tools.join(", ")),
                    is_error: false,
                });
            }
            SessionCommand::ConfirmBash { request_id, approved } => {
                if !self.bash_confirms.respond(&request_id, approved) {
                    warn!("bash confirm response for unknown request: {request_id}");
                }
            }
            SessionCommand::TodoResponse { request_id, response } => {
                if !self.todo_confirms.respond(&request_id, response) {
                    warn!("todo response for unknown request: {request_id}");
                }
            }
            SessionCommand::SlashCommand { command, args } => {
                info!("slash command: /{command} {args}");
                self.emit(DaemonEvent::SystemMessage {
                    text: format!("Slash command /{command} {args}"),
                    is_error: false,
                });
            }
            SessionCommand::ReplayHistory => {
                self.replay_history();
            }
            SessionCommand::GetCapabilities => {
                self.emit(DaemonEvent::Capabilities {
                    capabilities: self.capabilities.clone(),
                });
            }
            SessionCommand::Disconnect => {
                debug!("client disconnected");
            }
        }
    }

    /// Drain pending events. Called in a loop by the transport layer.
    ///
    /// In daemon mode, reads from the internal agent event receiver.
    /// In embedded mode, events must be fed via [`feed_event`] first.
    pub fn drain_events(&mut self) -> Vec<DaemonEvent> {
        // Drain agent events from internal receiver (daemon mode).
        // Collect into a Vec to avoid borrowing event_rx and self simultaneously.
        let events: Vec<AgentEvent> = if let Some(ref mut rx) = self.event_rx {
            let mut buf = Vec::new();
            while let Ok(event) = rx.try_recv() {
                buf.push(event);
            }
            buf
        } else {
            Vec::new()
        };
        for event in &events {
            self.process_agent_event(event);
        }
        std::mem::take(&mut self.outgoing)
    }

    /// Take accumulated outgoing events without draining the internal
    /// receiver. Used in embedded mode after calling [`feed_event`].
    pub fn take_outgoing(&mut self) -> Vec<DaemonEvent> {
        std::mem::take(&mut self.outgoing)
    }

    /// Feed a single agent event for processing (embedded mode).
    ///
    /// Performs audit tracking, session persistence, lifecycle hooks,
    /// loop output accumulation, and DaemonEvent translation — the same
    /// processing that `drain_events` does internally.
    pub fn feed_event(&mut self, event: &AgentEvent) {
        self.process_agent_event(event);
    }

    /// Check if the agent is currently processing a prompt.
    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Get the session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get the current model.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Graceful shutdown.
    pub async fn shutdown(&mut self) {
        if self.busy
            && let Some(ref mut agent) = self.agent
        {
            agent.abort();
        }
        if let Some(ref pipeline) = self.hook_pipeline {
            debug!("firing SessionEnd hook");
            let payload = clankers_hooks::HookPayload::session(&self.session_id, "");
            let _ = pipeline.fire(clankers_hooks::HookPoint::SessionEnd, &payload).await;
        }
        info!("session controller shut down: {}", self.session_id);
    }

    // ── Internal ─────────────────────────────────────────────────

    /// Handle a prompt command (daemon mode only).
    async fn handle_prompt(&mut self, text: String, images: Vec<clankers_protocol::ImageData>) {
        if self.agent.is_none() {
            warn!("handle_prompt called in embedded mode");
            return;
        }

        if self.busy {
            self.outgoing.push(DaemonEvent::SystemMessage {
                text: "A prompt is already in progress".to_string(),
                is_error: true,
            });
            return;
        }

        self.busy = true;
        self.outgoing.push(DaemonEvent::AgentStart);

        // Take the agent out to avoid borrow conflicts with self
        let mut agent = self.agent.take().unwrap();

        let result = if images.is_empty() {
            agent.prompt(&text).await
        } else {
            let image_content: Vec<Content> = images
                .into_iter()
                .map(|img| Content::Image {
                    source: clankers_provider::message::ImageSource::Base64 {
                        media_type: img.media_type,
                        data: img.data,
                    },
                })
                .collect();
            agent.prompt_with_images(&text, image_content).await
        };

        // Put the agent back
        self.agent = Some(agent);
        self.busy = false;

        match result {
            Ok(()) => {
                self.emit(DaemonEvent::PromptDone { error: None });
            }
            Err(AgentError::Cancelled) => {
                self.emit(DaemonEvent::PromptDone {
                    error: Some("cancelled".to_string()),
                });
            }
            Err(e) => {
                self.emit(DaemonEvent::PromptDone {
                    error: Some(e.to_string()),
                });
            }
        }
    }

    /// Process a single agent event into zero or more daemon events.
    fn process_agent_event(&mut self, event: &AgentEvent) {
        // 1. Audit tracking
        self.audit.process_event(event);

        // 2. Track tool call names
        if let AgentEvent::ToolCall { call_id, tool_name, .. } = event {
            self.tool_call_names.insert(call_id.clone(), tool_name.clone());

            // Check signal_loop_success tool
            if tool_name == "signal_loop_success" {
                self.signal_loop_break();
            }
        }

        // 3. Accumulate tool output for loop break conditions
        if let AgentEvent::ToolExecutionEnd { result, .. } = event {
            for content in &result.content {
                if let clankers_agent::ToolResultContent::Text { text } = content {
                    if !self.loop_turn_output.is_empty() {
                        self.loop_turn_output.push('\n');
                    }
                    self.loop_turn_output.push_str(text);
                }
            }
        }

        // 4. Persist to session
        self.persist_event(event);

        // 5. Translate to DaemonEvent
        if let Some(daemon_event) = agent_event_to_daemon_event(event) {
            self.outgoing.push(daemon_event);
        }

        // 6. Fire lifecycle hooks
        self.fire_lifecycle_hooks(event);
    }

    /// Fire lifecycle hooks for session and turn events.
    fn fire_lifecycle_hooks(&self, event: &AgentEvent) {
        let Some(ref pipeline) = self.hook_pipeline else {
            return;
        };

        let session_id = self.session_id.clone();
        match event {
            AgentEvent::SessionStart { session_id: sid } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::SessionStart,
                    clankers_hooks::HookPayload::session("session-start", sid),
                );
            }
            AgentEvent::SessionShutdown { session_id: sid } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::SessionEnd,
                    clankers_hooks::HookPayload::session("session-end", sid),
                );
            }
            AgentEvent::TurnStart { .. } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::TurnStart,
                    clankers_hooks::HookPayload::empty("turn-start", &session_id),
                );
            }
            AgentEvent::TurnEnd { .. } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::TurnEnd,
                    clankers_hooks::HookPayload::empty("turn-end", &session_id),
                );
            }
            AgentEvent::ModelChange { from, to, reason } => {
                pipeline.fire_async(
                    clankers_hooks::HookPoint::ModelChange,
                    clankers_hooks::HookPayload::model_change(
                        "model-change",
                        &session_id,
                        from,
                        to,
                        reason,
                    ),
                );
            }
            _ => {}
        }
    }

    /// Replay conversation history to a newly-attached client (daemon mode).
    fn replay_history(&mut self) {
        if let Some(ref agent) = self.agent {
            for msg in agent.messages() {
                let block = serde_json::to_value(msg).unwrap_or_default();
                self.outgoing.push(DaemonEvent::HistoryBlock { block });
            }
        }
        self.emit(DaemonEvent::HistoryEnd);
    }

    /// Convert serialized messages to agent messages for seeding.
    fn convert_seed_messages(
        &self,
        messages: &[clankers_protocol::SerializedMessage],
    ) -> Vec<clankers_message::AgentMessage> {
        use clankers_message::{
            AgentMessage, AssistantMessage, Content, MessageId, StopReason, UserMessage,
        };

        messages
            .iter()
            .filter_map(|msg| {
                let id = MessageId::generate();
                let now = chrono::Utc::now();
                match msg.role.as_str() {
                    "user" => Some(AgentMessage::User(UserMessage {
                        id,
                        content: vec![Content::Text {
                            text: msg.content.clone(),
                        }],
                        timestamp: now,
                    })),
                    "assistant" => Some(AgentMessage::Assistant(AssistantMessage {
                        id,
                        content: vec![Content::Text {
                            text: msg.content.clone(),
                        }],
                        model: msg.model.clone().unwrap_or_default(),
                        usage: Default::default(),
                        stop_reason: StopReason::Stop,
                        timestamp: now,
                    })),
                    other => {
                        warn!("skipping unknown role in seed message: {other}");
                        None
                    }
                }
            })
            .collect()
    }

    /// Check if auto-test should run after a prompt completes. Returns a
    /// prompt string to send to the agent, or None.
    pub fn maybe_auto_test(&mut self) -> Option<String> {
        if !self.auto_test_enabled {
            return None;
        }
        if self.auto_test_in_progress {
            return None;
        }
        if self.active_loop_id.is_some() {
            return None;
        }
        let cmd = self.auto_test_command.as_ref()?;
        self.auto_test_in_progress = true;
        Some(format!(
            "Run `{cmd}` and fix any failures. Do not ask for confirmation."
        ))
    }

    /// Clear the auto-test guard (call after the auto-test prompt completes).
    pub fn clear_auto_test(&mut self) {
        self.auto_test_in_progress = false;
    }

    /// Determine what to do after a prompt completes (embedded mode).
    ///
    /// Call this from the TUI's `handle_task_results` after receiving
    /// `PromptDone(None)` and confirming there's no queued user prompt.
    /// Returns the action the TUI should take.
    pub fn check_post_prompt(&mut self) -> PostPromptAction {
        // Loop continuation takes priority
        if self.active_loop_id.is_some()
            && let Some(prompt) = self.maybe_continue_loop()
        {
            return PostPromptAction::ContinueLoop(prompt);
        }

        // Auto-test
        if let Some(prompt) = self.maybe_auto_test() {
            return PostPromptAction::RunAutoTest(prompt);
        }
        self.clear_auto_test();

        PostPromptAction::None
    }

    /// Sync loop state from the TUI's loop_status.
    ///
    /// Called before `check_post_prompt()` to ensure the controller's
    /// loop engine matches the TUI's `/loop` command state.
    pub fn sync_loop_from_tui(
        &mut self,
        loop_status: Option<&clankers_tui_types::LoopDisplayState>,
    ) {
        match (loop_status, &self.active_loop_id) {
            // TUI has loop but controller doesn't → register it
            (Some(ls), None) => {
                let config = loop_mode::LoopConfig {
                    name: ls.name.clone(),
                    prompt: ls.prompt.clone(),
                    max_iterations: ls.max_iterations,
                    break_text: ls.break_text.clone(),
                };
                self.start_loop(config);
            }
            // TUI cleared loop but controller still has one → stop it
            (None, Some(_)) => {
                if let Some(ref id) = self.active_loop_id {
                    self.loop_engine.stop(id);
                    self.loop_engine.remove(id);
                }
                self.active_loop_id = None;
                self.loop_turn_output.clear();
            }
            // Both in sync (or neither has a loop)
            _ => {}
        }
    }

    /// Get the current loop iteration count (for TUI display sync).
    pub fn loop_iteration(&self) -> Option<u32> {
        self.active_loop_id
            .as_ref()
            .and_then(|id| self.loop_engine.get(id))
            .map(|s| s.current_iteration)
    }

    /// Notify the controller that a prompt completed (embedded mode).
    ///
    /// Updates busy state. Called from the TUI when `TaskResult::PromptDone`
    /// is received, before calling `check_post_prompt()`.
    pub fn notify_prompt_done(&mut self, had_error: bool) {
        self.busy = false;
        if had_error && self.active_loop_id.is_some() {
            self.finish_loop("failed (error)");
        }
    }

    /// Update the session ID (e.g., after session resume).
    pub fn set_session_id(&mut self, id: String) {
        self.session_id = id;
    }

    /// Update the model name (e.g., after model switch from TUI).
    pub fn set_model_name(&mut self, model: String) {
        self.model = model;
    }

    /// Update auto-test settings from the TUI.
    pub fn set_auto_test(&mut self, enabled: bool, command: Option<String>) {
        self.auto_test_enabled = enabled;
        self.auto_test_command = command;
    }

    /// Access the session manager (for branch/merge operations).
    pub fn session_manager(&self) -> Option<&SessionManager> {
        self.session_manager.as_ref()
    }

    /// Mutably access the session manager (for branch/merge operations).
    pub fn session_manager_mut(&mut self) -> Option<&mut SessionManager> {
        self.session_manager.as_mut()
    }

    /// Queue an outgoing event.
    fn emit(&mut self, event: DaemonEvent) {
        self.outgoing.push(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ControllerConfig;

    /// Minimal mock provider for controller tests (no actual LLM calls).
    struct MockProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for MockProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clankers_provider::streaming::StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            Ok(())
        }
        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }
        fn name(&self) -> &str {
            "mock"
        }
    }

    fn make_test_controller() -> SessionController {
        let provider = Arc::new(MockProvider);
        let agent = Agent::new(
            provider,
            vec![],
            clankers_config::settings::Settings::default(),
            "test-model".to_string(),
            "You are a test assistant.".to_string(),
        );

        let config = ControllerConfig {
            session_id: "test-session".to_string(),
            model: "test-model".to_string(),
            ..Default::default()
        };

        SessionController::new(agent, config)
    }

    #[tokio::test]
    async fn test_handle_abort() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::Abort).await;

        let events = ctrl.drain_events();
        assert!(
            events.iter().any(
                |e| matches!(e, DaemonEvent::SystemMessage { text, is_error: false } if text.contains("cancelled"))
            )
        );
    }

    #[tokio::test]
    async fn test_handle_clear_history() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ClearHistory).await;

        let events = ctrl.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("cleared")))
        );
    }

    #[tokio::test]
    async fn test_handle_set_model() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetModel {
            model: "opus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::ModelChanged {
                from,
                to,
                ..
            } if from == "test-model" && to == "opus"
        )));
        assert_eq!(ctrl.model(), "opus");
    }

    #[tokio::test]
    async fn test_handle_get_system_prompt() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::GetSystemPrompt).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::SystemPromptResponse { prompt } if prompt == "You are a test assistant."
        )));
    }

    #[tokio::test]
    async fn test_handle_get_capabilities_none() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::GetCapabilities).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::Capabilities { capabilities: None })));
    }

    #[tokio::test]
    async fn test_handle_get_capabilities_some() {
        let mut ctrl = make_test_controller();
        ctrl.capabilities = Some(vec!["read".to_string(), "grep".to_string()]);
        ctrl.handle_command(SessionCommand::GetCapabilities).await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(
            e,
            DaemonEvent::Capabilities { capabilities: Some(caps) } if caps.len() == 2
        )));
    }

    #[tokio::test]
    async fn test_reject_concurrent_prompt() {
        let mut ctrl = make_test_controller();
        ctrl.busy = true;

        ctrl.handle_command(SessionCommand::Prompt {
            text: "hello".to_string(),
            images: vec![],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
    }

    #[tokio::test]
    async fn test_replay_history() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ReplayHistory).await;

        let events = ctrl.drain_events();
        // Should end with HistoryEnd
        assert!(events.last().is_some_and(|e| matches!(e, DaemonEvent::HistoryEnd)));
    }

    #[tokio::test]
    async fn test_confirm_bash_unknown_request() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::ConfirmBash {
            request_id: "nonexistent".to_string(),
            approved: true,
        })
        .await;
        // Should just log a warning, not crash
        let events = ctrl.drain_events();
        assert!(events.is_empty());
    }

    #[test]
    fn test_not_busy_initially() {
        let ctrl = make_test_controller();
        assert!(!ctrl.is_busy());
    }

    #[test]
    fn test_session_id() {
        let ctrl = make_test_controller();
        assert_eq!(ctrl.session_id(), "test-session");
    }

    #[tokio::test]
    async fn test_set_thinking_level_valid() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "high".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(
            |e| matches!(e, DaemonEvent::SystemMessage { text, is_error: false } if text.contains("high"))
        ));
    }

    #[tokio::test]
    async fn test_set_thinking_level_invalid() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SetThinkingLevel {
            level: "bogus".to_string(),
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events
            .iter()
            .any(|e| matches!(e, DaemonEvent::SystemMessage { is_error: true, .. })));
    }

    #[tokio::test]
    async fn test_seed_messages() {
        let mut ctrl = make_test_controller();
        ctrl.handle_command(SessionCommand::SeedMessages {
            messages: vec![
                clankers_protocol::SerializedMessage {
                    role: "user".to_string(),
                    content: "hello".to_string(),
                    model: None,
                    timestamp: None,
                },
                clankers_protocol::SerializedMessage {
                    role: "assistant".to_string(),
                    content: "hi".to_string(),
                    model: Some("opus".to_string()),
                    timestamp: None,
                },
            ],
        })
        .await;

        let events = ctrl.drain_events();
        assert!(events.iter().any(
            |e| matches!(e, DaemonEvent::SystemMessage { text, .. } if text.contains("2"))
        ));
        // Agent should have 2 messages now
        assert_eq!(ctrl.agent.as_ref().unwrap().messages().len(), 2);
    }

    #[test]
    fn test_auto_test_disabled() {
        let mut ctrl = make_test_controller();
        assert!(ctrl.maybe_auto_test().is_none());
    }

    #[test]
    fn test_auto_test_fires() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());

        let prompt = ctrl.maybe_auto_test();
        assert!(prompt.is_some());
        assert!(prompt.unwrap().contains("cargo test"));

        // Second call blocked (in progress)
        assert!(ctrl.maybe_auto_test().is_none());

        // After clearing, can fire again
        ctrl.clear_auto_test();
        assert!(ctrl.maybe_auto_test().is_some());
    }

    #[test]
    fn test_auto_test_blocked_during_loop() {
        let mut ctrl = make_test_controller();
        ctrl.auto_test_enabled = true;
        ctrl.auto_test_command = Some("cargo test".to_string());
        ctrl.active_loop_id = Some(clankers_loop::LoopId("test-loop".to_string()));

        assert!(ctrl.maybe_auto_test().is_none());
    }
}
