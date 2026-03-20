//! Transport-agnostic session controller for agent orchestration.
//!
//! The `SessionController` owns one agent session and accepts `SessionCommand`
//! inputs, emitting `DaemonEvent` outputs. It does not know about terminals,
//! sockets, or rendering — that's the client's job.
//!
//! This is the core piece extracted from `EventLoopRunner` that contains all
//! the non-TUI agent orchestration logic.

pub mod audit;
pub mod auto_test;
pub mod capability;
pub mod client;
pub mod command;
pub mod config;
pub mod confirm;
pub mod convert;
pub mod event_processing;
pub mod loop_mode;
pub mod persistence;
pub mod transport;

use std::collections::HashMap;
use std::sync::Arc;

use clankers_agent::Agent;
use clankers_agent::events::AgentEvent;
use clankers_hooks::HookPipeline;
use clanker_loop::LoopEngine;
use clanker_loop::LoopId;
use clankers_protocol::DaemonEvent;
use clankers_session::SessionManager;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;
use tracing::warn;

use self::audit::AuditTracker;
use self::config::ControllerConfig;
use self::confirm::ConfirmStore;

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
    pub(crate) agent: Option<Agent>,
    /// Receiver for agent events (None in embedded mode).
    pub(crate) event_rx: Option<broadcast::Receiver<AgentEvent>>,
    /// Session persistence.
    pub session_manager: Option<SessionManager>,
    /// Loop engine for loop/retry iteration.
    pub(crate) loop_engine: LoopEngine,
    /// Active loop ID.
    pub(crate) active_loop_id: Option<LoopId>,
    /// Accumulated tool output for break conditions.
    pub(crate) loop_turn_output: String,
    /// Lifecycle hooks pipeline.
    pub(crate) hook_pipeline: Option<Arc<HookPipeline>>,
    /// Tool call timing and leak detection.
    pub(crate) audit: AuditTracker,
    /// Maps call_id → tool_name for tool result persistence.
    pub(crate) tool_call_names: HashMap<String, String>,
    /// Pending bash confirmations.
    pub(crate) bash_confirms: ConfirmStore<bool>,
    /// Pending todo responses.
    pub(crate) todo_confirms: ConfirmStore<serde_json::Value>,
    /// Queued outgoing events.
    pub(crate) outgoing: Vec<DaemonEvent>,
    /// Whether a prompt is currently in progress.
    pub(crate) busy: bool,
    /// Prevents recursive auto-test triggers.
    pub(crate) auto_test_in_progress: bool,
    /// Auto-test command from settings.
    pub(crate) auto_test_command: Option<String>,
    /// Whether auto-test is enabled.
    pub(crate) auto_test_enabled: bool,
    /// Session ID.
    pub(crate) session_id: String,
    /// Capability restrictions (None = full access).
    pub(crate) capabilities: Option<Vec<String>>,
    /// Maximum capabilities this session can have (immutable after creation).
    /// `None` = no ceiling (local owner). `SetCapabilities` commands are
    /// validated against this — the user can attenuate but never escalate.
    pub(crate) capability_ceiling: Option<Vec<String>>,
    /// Current model name.
    pub(crate) model: String,
    /// Currently disabled tools.
    pub(crate) disabled_tools: Vec<String>,
    /// Optional tool rebuilder for hot-reloading tools on toggle.
    pub(crate) tool_rebuilder: Option<Arc<dyn ToolRebuilder>>,
}

/// Trait for rebuilding the filtered tool set when disabled tools change.
pub trait ToolRebuilder: Send + Sync {
    fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>>;
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
            capabilities: config.capabilities.clone(),
            capability_ceiling: config.capability_ceiling.or(config.capabilities),
            model,
            disabled_tools: Vec::new(),
            tool_rebuilder: None,
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
            capabilities: config.capabilities.clone(),
            capability_ceiling: config.capability_ceiling.or(config.capabilities),
            model,
            disabled_tools: Vec::new(),
            tool_rebuilder: None,
        }
    }

    /// Set the tool rebuilder for hot-reloading tools on toggle.
    pub fn set_tool_rebuilder(&mut self, rebuilder: Arc<dyn ToolRebuilder>) {
        self.tool_rebuilder = Some(rebuilder);
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
        // Flush any unsaved messages to the session file before shutting down.
        // This catches in-progress turns that haven't hit AgentEnd yet.
        if let Some(ref mut agent) = self.agent
            && let Some(ref mut sm) = self.session_manager
        {
            let messages = agent.messages().to_vec();
            let mut flushed = 0;
            for msg in &messages {
                if !sm.is_persisted(msg.id()) {
                    let parent = sm.active_leaf_id().cloned();
                    if let Err(e) = sm.append_message(msg.clone(), parent) {
                        warn!("shutdown flush failed: {e}");
                    } else {
                        flushed += 1;
                    }
                }
            }
            if flushed > 0 {
                info!("session {}: flushed {flushed} unsaved messages on shutdown", self.session_id);
            }
        }
        if let Some(ref pipeline) = self.hook_pipeline {
            debug!("firing SessionEnd hook");
            let payload = clankers_hooks::HookPayload::session(&self.session_id, "");
            let _ = pipeline.fire(clankers_hooks::HookPoint::SessionEnd, &payload).await;
        }
        info!("session controller shut down: {}", self.session_id);
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

    /// Register a pending bash confirmation request.
    ///
    /// Returns a `(request_id, receiver)` pair. The daemon actor emits a
    /// `ConfirmRequest` event with the `request_id`, and when the client
    /// responds with `ConfirmBash`, the receiver resolves.
    pub fn register_bash_confirm(&mut self) -> (String, tokio::sync::oneshot::Receiver<bool>) {
        self.bash_confirms.register()
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
pub(crate) mod test_helpers {
    use std::sync::Arc;
    use super::*;

    /// Minimal mock provider for controller tests (no actual LLM calls).
    pub struct MockProvider;

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

    pub fn make_test_controller() -> SessionController {
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
}

#[cfg(test)]
mod tests {
    use crate::test_helpers::make_test_controller;



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


}
