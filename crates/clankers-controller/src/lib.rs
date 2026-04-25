//! Transport-agnostic session controller for agent orchestration.
//!
//! The `SessionController` owns one agent session and accepts `SessionCommand`
//! inputs, emitting `DaemonEvent` outputs. It does not know about terminals,
//! sockets, or rendering — that's the client's job.
//!
//! This is the core piece extracted from `EventLoopRunner` that contains all
//! the non-TUI agent orchestration logic.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

pub mod audit;
pub mod auto_test;
pub mod capability;
pub mod client;
pub mod command;
pub mod config;
pub mod confirm;
pub mod convert;
pub mod core_effects;
pub(crate) mod core_engine_composition;
pub mod event_processing;
pub mod loop_mode;
pub mod metrics_capture;
pub mod persistence;
pub mod transport;
pub mod transport_convert;

use std::collections::HashMap;
use std::sync::Arc;

use clanker_loop::LoopEngine;
use clanker_loop::LoopId;
use clankers_agent::Agent;
use clankers_agent::events::AgentEvent;
use clankers_core::CoreState;
use clankers_hooks::HookPipeline;
use clankers_protocol::DaemonEvent;
use clankers_session::SessionManager;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;
use tracing::warn;

use self::audit::AuditTracker;
use self::config::ControllerConfig;
use self::confirm::ConfirmStore;

/// Controller-owned identity for pending shell work correlated back into the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PendingWorkId(u64);

impl PendingWorkId {
    pub(crate) fn from_core(effect_id: clankers_core::CoreEffectId) -> Self {
        Self(effect_id.0)
    }

    #[cfg(test)]
    pub(crate) fn from_raw(raw: u64) -> Self {
        Self(raw)
    }

    #[cfg(test)]
    pub(crate) fn raw(self) -> u64 {
        self.0
    }

    pub(crate) fn into_core(self) -> clankers_core::CoreEffectId {
        clankers_core::CoreEffectId(self.0)
    }
}

/// Shell-native completion outcome reported back to the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellPromptCompletion {
    Succeeded,
    Cancelled,
    Failed { message: String },
}

impl ShellPromptCompletion {
    pub fn cancelled() -> Self {
        Self::Cancelled
    }

    pub fn failed(message: impl Into<String>) -> Self {
        Self::Failed {
            message: message.into(),
        }
    }

    pub(crate) fn to_core(&self) -> clankers_core::CompletionStatus {
        match self {
            Self::Succeeded => clankers_core::CompletionStatus::Succeeded,
            Self::Cancelled => clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Cancelled),
            Self::Failed { message } => {
                clankers_core::CompletionStatus::Failed(clankers_core::CoreFailure::Message(message.clone()))
            }
        }
    }
}

/// Shell-native follow-up dispatch result reported back to the controller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellFollowUpDispatch {
    Accepted,
    Rejected { message: String },
}

impl ShellFollowUpDispatch {
    pub fn rejected(message: impl Into<String>) -> Self {
        Self::Rejected {
            message: message.into(),
        }
    }

    pub(crate) fn to_core(&self) -> clankers_core::FollowUpDispatchStatus {
        match self {
            Self::Accepted => clankers_core::FollowUpDispatchStatus::Accepted,
            Self::Rejected { message } => {
                clankers_core::FollowUpDispatchStatus::Rejected(clankers_core::CoreFailure::Message(message.clone()))
            }
        }
    }
}

/// Action the TUI should take after a prompt completes.
#[derive(Debug)]
pub enum PostPromptAction {
    /// No continuation — prompt is done.
    None,
    /// Replay a queued user prompt that already lives in shell state.
    ReplayQueuedPrompt,
    /// Continue a loop iteration with this prompt.
    ContinueLoop {
        pending_work_id: PendingWorkId,
        prompt: String,
    },
    /// Run an auto-test with this prompt.
    RunAutoTest {
        pending_work_id: PendingWorkId,
        prompt: String,
    },
}

/// Transport-agnostic orchestrator that owns one agent session.
///
/// Accepts `SessionCommand`, emits `DaemonEvent`. Does not know about
/// terminals, sockets, or rendering.
///
/// Two modes:
/// - **Daemon mode** (`new`): owns the Agent, drives prompts directly.
/// - **Embedded mode** (`new_embedded`): no agent ownership. Events fed via `feed_event()`, agent
///   managed externally by `agent_task`.
#[allow(dead_code)] // Fields used incrementally as phases are implemented
pub struct SessionController {
    /// The agent instance (None in embedded mode).
    pub(crate) agent: Option<Agent>,
    /// Receiver for agent events (None in embedded mode).
    pub(crate) event_rx: Option<broadcast::Receiver<AgentEvent>>,
    /// Session persistence.
    pub session_manager: Option<SessionManager>,
    /// Authoritative reducer state for the migrated no_std slice.
    pub(crate) core_state: CoreState,
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
    /// Metrics collector (aggregates session metrics from agent events).
    pub(crate) metrics: metrics_capture::MetricsCollector,
    /// Full-text search index for session content (optional).
    pub(crate) search_index: Option<Arc<clankers_db::search_index::SearchIndex>>,
}

/// Trait for rebuilding the filtered tool set when disabled tools change.
pub trait ToolRebuilder: Send + Sync {
    fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>>;
}

impl SessionController {
    /// Create a new controller that owns the Agent (daemon mode).
    pub fn new(mut agent: Agent, config: ControllerConfig) -> Self {
        agent.set_session_id(config.session_id.clone());
        let event_rx = agent.subscribe();
        let model = config.model.clone();
        let metrics = metrics_capture::MetricsCollector::new(config.session_id.clone());

        Self {
            agent: Some(agent),
            event_rx: Some(event_rx),
            session_manager: config.session_manager,
            core_state: CoreState {
                auto_test_enabled: config.auto_test_enabled,
                auto_test_command: config.auto_test_command.clone(),
                ..CoreState::default()
            },
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
            metrics,
            search_index: None,
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
        let metrics = metrics_capture::MetricsCollector::new(config.session_id.clone());

        Self {
            agent: None,
            event_rx: None,
            session_manager: config.session_manager,
            core_state: CoreState {
                auto_test_enabled: config.auto_test_enabled,
                auto_test_command: config.auto_test_command.clone(),
                ..CoreState::default()
            },
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
            metrics,
            search_index: None,
        }
    }

    /// Set the tool rebuilder for hot-reloading tools on toggle.
    pub fn set_tool_rebuilder(&mut self, rebuilder: Arc<dyn ToolRebuilder>) {
        self.tool_rebuilder = Some(rebuilder);
    }

    pub fn set_search_index(&mut self, index: Arc<clankers_db::search_index::SearchIndex>) {
        self.search_index = Some(index);
    }

    /// Snapshot the current tool list as protocol metadata.
    pub fn current_tool_infos(&self) -> Vec<clankers_protocol::ToolInfo> {
        self.agent
            .as_ref()
            .map(|agent| {
                agent
                    .tools()
                    .iter()
                    .map(|tool| {
                        let def = tool.definition();
                        clankers_protocol::ToolInfo {
                            name: def.name.clone(),
                            description: def.description.clone(),
                            source: tool.source().to_string(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Rebuild the filtered tool list from the current rebuilder and disabled set.
    ///
    /// Returns true when the tool inventory changed.
    pub fn refresh_tools(&mut self) -> bool {
        let Some(rebuilder) = self.tool_rebuilder.as_ref() else {
            return false;
        };
        if self.agent.is_none() {
            return false;
        }

        let before = self.current_tool_infos();
        let rebuilt = rebuilder.rebuild_filtered(&self.disabled_tools);
        let after: Vec<clankers_protocol::ToolInfo> = rebuilt
            .iter()
            .map(|tool| {
                let def = tool.definition();
                clankers_protocol::ToolInfo {
                    name: def.name.clone(),
                    description: def.description.clone(),
                    source: tool.source().to_string(),
                }
            })
            .collect();
        if before == after {
            return false;
        }

        if let Some(agent) = self.agent.as_mut() {
            agent.set_tools(rebuilt);
        }
        true
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
        self.session_id = id.clone();
        if let Some(ref mut agent) = self.agent {
            agent.set_session_id(id);
        }
    }

    /// Update the model name (e.g., after model switch from TUI).
    pub fn set_model_name(&mut self, model: String) {
        self.model = model;
    }

    /// Mutable access to the metrics collector for plugin dispatch and other instrumenters.
    pub fn metrics_mut(&mut self) -> &mut metrics_capture::MetricsCollector {
        &mut self.metrics
    }

    /// Read access to the current session metrics summary.
    pub fn metrics_summary(&self) -> &clankers_db::metrics::types::SessionMetricsSummary {
        self.metrics.summary()
    }

    /// Update auto-test settings from the TUI.
    pub fn set_auto_test(&mut self, enabled: bool, command: Option<String>) {
        self.auto_test_enabled = enabled;
        self.auto_test_command = command.clone();
        self.core_state.auto_test_enabled = enabled;
        self.core_state.auto_test_command = command;
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

    /// Apply the authoritative core-state snapshot back onto controller mirrors.
    fn apply_core_state(&mut self, next_state: CoreState) {
        self.busy = next_state.busy;
        self.disabled_tools = next_state.disabled_tools.clone();
        self.auto_test_enabled = next_state.auto_test_enabled;
        self.auto_test_command = next_state.auto_test_command.clone();
        self.auto_test_in_progress = next_state.auto_test_in_progress;
        self.core_state = next_state;
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
    use std::sync::Arc;
    use std::sync::Mutex;

    use crate::ToolRebuilder;
    use crate::test_helpers::make_test_controller;

    struct StubTool {
        definition: clankers_agent::ToolDefinition,
        source: &'static str,
    }

    #[async_trait::async_trait]
    impl clankers_agent::Tool for StubTool {
        fn definition(&self) -> &clankers_agent::ToolDefinition {
            &self.definition
        }

        async fn execute(
            &self,
            _ctx: &clankers_agent::ToolContext,
            _params: serde_json::Value,
        ) -> clankers_agent::ToolResult {
            clankers_agent::ToolResult::text("ok")
        }

        fn source(&self) -> &str {
            self.source
        }
    }

    struct StubRebuilder {
        tools: Vec<Arc<dyn clankers_agent::Tool>>,
    }

    impl ToolRebuilder for StubRebuilder {
        fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>> {
            self.tools
                .iter()
                .filter(|tool| !disabled.iter().any(|name| name == &tool.definition().name))
                .cloned()
                .collect()
        }
    }

    struct DynamicRebuilder {
        tools: Arc<Mutex<Vec<Arc<dyn clankers_agent::Tool>>>>,
    }

    impl ToolRebuilder for DynamicRebuilder {
        fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>> {
            self.tools
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .filter(|tool| !disabled.iter().any(|name| name == &tool.definition().name))
                .cloned()
                .collect()
        }
    }

    fn stub_tool(name: &str, source: &'static str) -> Arc<dyn clankers_agent::Tool> {
        Arc::new(StubTool {
            definition: clankers_agent::ToolDefinition {
                name: name.to_string(),
                description: format!("stub {name}"),
                input_schema: serde_json::json!({"type": "object"}),
            },
            source,
        })
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

    #[test]
    fn test_controller_sets_agent_session_id() {
        let ctrl = make_test_controller();
        let agent = ctrl.agent.as_ref().expect("controller should own an agent");
        assert_eq!(agent.session_id(), "test-session");
    }

    #[test]
    fn test_controller_updates_agent_session_id() {
        let mut ctrl = make_test_controller();
        ctrl.set_session_id("resumed-session".to_string());
        let agent = ctrl.agent.as_ref().expect("controller should own an agent");
        assert_eq!(ctrl.session_id(), "resumed-session");
        assert_eq!(agent.session_id(), "resumed-session");
    }

    #[test]
    fn refresh_tools_updates_inventory_and_sources() {
        let mut ctrl = make_test_controller();
        ctrl.set_tool_rebuilder(Arc::new(StubRebuilder {
            tools: vec![stub_tool("stdio_echo", "stdio-plugin")],
        }));

        assert!(ctrl.refresh_tools());
        assert_eq!(ctrl.current_tool_infos(), vec![clankers_protocol::ToolInfo {
            name: "stdio_echo".to_string(),
            description: "stub stdio_echo".to_string(),
            source: "stdio-plugin".to_string(),
        }]);
        assert!(!ctrl.refresh_tools());
    }

    #[test]
    fn refresh_tools_respects_disabled_tools() {
        let mut ctrl = make_test_controller();
        ctrl.set_tool_rebuilder(Arc::new(StubRebuilder {
            tools: vec![stub_tool("stdio_echo", "stdio-plugin")],
        }));

        assert!(ctrl.refresh_tools());
        ctrl.disabled_tools = vec!["stdio_echo".to_string()];
        assert!(ctrl.refresh_tools());
        assert!(ctrl.current_tool_infos().is_empty());
    }

    #[test]
    fn refresh_tools_keeps_disabled_stdio_tool_hidden_when_rebuilder_reintroduces_it() {
        let mut ctrl = make_test_controller();
        let tools = Arc::new(Mutex::new(vec![stub_tool("stdio_echo", "stdio-plugin")]));
        ctrl.set_tool_rebuilder(Arc::new(DynamicRebuilder {
            tools: Arc::clone(&tools),
        }));

        assert!(ctrl.refresh_tools());
        ctrl.disabled_tools = vec!["stdio_echo".to_string()];
        assert!(ctrl.refresh_tools());
        assert!(ctrl.current_tool_infos().is_empty());

        tools.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clear();
        assert!(!ctrl.refresh_tools());
        assert!(ctrl.current_tool_infos().is_empty());

        tools
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(stub_tool("stdio_echo", "stdio-plugin"));
        assert!(!ctrl.refresh_tools());
        assert!(ctrl.current_tool_infos().is_empty());
    }
}
