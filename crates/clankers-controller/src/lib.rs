//! Transport-agnostic session controller for agent orchestration.
//!
//! The `SessionController` owns one agent session and accepts `SessionCommand`
//! inputs, emitting `DaemonEvent` outputs. It does not know about terminals,
//! sockets, or rendering — that's the client's job.
//!
//! This is the core piece extracted from `EventLoopRunner` that contains all
//! the non-TUI agent orchestration logic.
#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", feature(register_tool), register_tool(tigerstyle))]
#![cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        tigerstyle::assertion_density,
        tigerstyle::compound_condition,
        tigerstyle::ambient_clock,
        tigerstyle::usize_in_public_api,
        tigerstyle::sentinel_fallback,
        tigerstyle::bool_naming,
        tigerstyle::no_panic,
        tigerstyle::no_unwrap,
        tigerstyle::unbounded_channel,
        tigerstyle::explicit_defaults,
        tigerstyle::raw_arithmetic_overflow,
        tigerstyle::unbounded_collection_growth,
        tigerstyle::ambiguous_params,
        tigerstyle::unchecked_narrowing,
        tigerstyle::unbounded_loop,
        reason = "controller is transport/orchestration shell with stable daemon protocol behavior covered by controller and runtime tests"
    )
)]

pub mod audit;
pub mod auto_test;
pub mod capability;
pub mod client;
pub mod command;
pub(crate) mod command_images;
pub(crate) mod command_responsibility;
pub(crate) mod command_thinking;
pub mod config;
pub mod confirm;
pub mod convert;
pub mod core_effects;
pub(crate) mod core_engine_composition;
pub(crate) mod domain_event;
pub(crate) mod effect_interpretation;
pub mod event_processing;
pub mod hooks;
pub mod loop_mode;
pub mod metrics_capture;
pub mod persistence;
pub mod persistence_service;
pub mod runtime_adapter;
pub mod session_ledger;
pub mod transport;
pub mod transport_convert;

use std::collections::HashMap;
use std::sync::Arc;

use clanker_loop::LoopEngine;
use clanker_loop::LoopId;
use clankers_agent::Agent;
use clankers_agent::events::AgentEvent;
use clankers_core::CoreState;
use clankers_protocol::DaemonEvent;
pub use hooks::ControllerHookData;
pub use hooks::ControllerHookPayload;
pub use hooks::ControllerHookPoint;
pub use hooks::ControllerHookSafeError;
pub use hooks::ControllerHookService;
pub use hooks::ControllerHookStatus;
pub use hooks::ControllerHookUsage;
pub use hooks::ControllerHookVerdict;
pub use persistence_service::ControllerPersistenceService;
pub use runtime_adapter::AgentBackedRuntimeAdapter;
pub use runtime_adapter::AgentRuntimePromptRequest;
pub use runtime_adapter::ControllerRuntimeAdapter;
pub use runtime_adapter::FakeRuntimeAdapter;
pub use runtime_adapter::RuntimeControlRequest;
pub use runtime_adapter::RuntimeControlResult;
pub use runtime_adapter::RuntimePromptCompletion;
pub use runtime_adapter::RuntimePromptRequest;
pub use runtime_adapter::RuntimePromptResult;
pub use session_ledger::ControllerSessionLedger;
use tokio::sync::broadcast;
use tracing::debug;
use tracing::info;

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
/// - **Daemon mode** (`new`): compatibility assembly owns an `Agent`, and production command
///   branches construct an `AgentBackedRuntimeAdapter` at the command boundary for concrete
///   prompt/control operations.
/// - **Embedded mode** (`new_embedded`): no agent ownership. Events fed via `feed_event()`, agent
///   managed externally by `agent_task`.
#[allow(dead_code)] // Fields used incrementally as phases are implemented
pub struct SessionController {
    /// Compatibility-owned agent instance (None in embedded mode).
    ///
    /// Production command handling must wrap this value in
    /// `AgentBackedRuntimeAdapter` rather than mutating it directly. The
    /// convergence condition for removing this field is root/daemon injection
    /// of the production runtime adapter instead of passing `Agent` into
    /// [`SessionController::new`].
    pub(crate) agent: Option<Agent>,
    /// Receiver for agent events (None in embedded mode).
    pub(crate) event_rx: Option<broadcast::Receiver<AgentEvent>>,
    /// Session persistence.
    pub session_ledger: Option<Box<dyn ControllerSessionLedger>>,
    /// Authoritative reducer state for the migrated no_std slice.
    pub(crate) core_state: CoreState,
    /// Loop engine for loop/retry iteration.
    pub(crate) loop_engine: LoopEngine,
    /// Active loop ID.
    pub(crate) active_loop_id: Option<LoopId>,
    /// Accumulated tool output for break conditions.
    pub(crate) loop_turn_output: String,
    /// Lifecycle hook service.
    pub(crate) hook_service: Option<Arc<dyn ControllerHookService>>,
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
    /// Optional host persistence side effects such as search indexing.
    pub(crate) persistence_service: Option<Arc<dyn ControllerPersistenceService>>,
}

/// Trait for rebuilding the filtered tool set when disabled tools change.
pub trait ToolRebuilder: Send + Sync {
    fn rebuild_filtered(&self, disabled: &[String]) -> Vec<Arc<dyn clankers_agent::Tool>>;
}

#[derive(Clone)]
struct ControllerAgentHookService {
    service: Arc<dyn ControllerHookService>,
}

impl ControllerAgentHookService {
    fn new(service: Arc<dyn ControllerHookService>) -> Self {
        Self { service }
    }
}

#[async_trait::async_trait]
impl clankers_agent::AgentHookService for ControllerAgentHookService {
    async fn fire(
        &self,
        point: clankers_agent::AgentHookPoint,
        payload: &clankers_agent::AgentHookPayload,
    ) -> clankers_agent::AgentHookVerdict {
        let hook_point = controller_hook_point_from_agent(point);
        let payload = controller_hook_payload_from_agent(payload);
        controller_hook_verdict_to_agent(self.service.fire(hook_point, &payload).await)
    }

    fn fire_async(&self, point: clankers_agent::AgentHookPoint, payload: clankers_agent::AgentHookPayload) {
        let hook_point = controller_hook_point_from_agent(point);
        let payload = controller_hook_payload_from_agent(&payload);
        self.service.fire_async(hook_point, payload);
    }
}

fn controller_hook_point_from_agent(point: clankers_agent::AgentHookPoint) -> ControllerHookPoint {
    match point {
        clankers_agent::AgentHookPoint::PrePrompt => ControllerHookPoint::PrePrompt,
        clankers_agent::AgentHookPoint::PostPrompt => ControllerHookPoint::PostPrompt,
        clankers_agent::AgentHookPoint::PreTurn => ControllerHookPoint::PreTurn,
        clankers_agent::AgentHookPoint::PostTurn => ControllerHookPoint::PostTurn,
    }
}

fn controller_hook_verdict_to_agent(verdict: ControllerHookVerdict) -> clankers_agent::AgentHookVerdict {
    match verdict {
        ControllerHookVerdict::Continue => clankers_agent::AgentHookVerdict::Continue,
        ControllerHookVerdict::Modify(value) => clankers_agent::AgentHookVerdict::Modify(value),
        ControllerHookVerdict::Deny { reason } => clankers_agent::AgentHookVerdict::Deny { reason },
    }
}

fn controller_hook_payload_from_agent(payload: &clankers_agent::AgentHookPayload) -> ControllerHookPayload {
    let data = match &payload.data {
        clankers_agent::AgentHookData::Prompt {
            prompt_id,
            text,
            system_prompt,
            status,
            error,
        } => ControllerHookData::Prompt {
            prompt_id: prompt_id.clone(),
            text: text.clone(),
            system_prompt: system_prompt.clone(),
            status: controller_hook_status_from_agent(*status),
            error: error.as_ref().map(controller_hook_error_from_agent),
        },
        clankers_agent::AgentHookData::Turn {
            prompt_id,
            model,
            prompt_text,
            message_count,
            tool_call_count,
            status,
            error,
            usage,
        } => ControllerHookData::Turn {
            prompt_id: prompt_id.clone(),
            model: model.clone(),
            prompt_text: prompt_text.clone(),
            message_count: *message_count,
            tool_call_count: *tool_call_count,
            status: controller_hook_status_from_agent(*status),
            error: error.as_ref().map(controller_hook_error_from_agent),
            usage: usage.as_ref().map(controller_hook_usage_from_agent),
        },
    };
    ControllerHookPayload {
        event_name: payload.event_name.clone(),
        session_id: payload.session_id.clone(),
        data,
    }
}

fn controller_hook_status_from_agent(status: clankers_agent::AgentHookStatus) -> ControllerHookStatus {
    match status {
        clankers_agent::AgentHookStatus::Pending => ControllerHookStatus::Pending,
        clankers_agent::AgentHookStatus::Success => ControllerHookStatus::Success,
        clankers_agent::AgentHookStatus::Denied => ControllerHookStatus::Denied,
        clankers_agent::AgentHookStatus::Error => ControllerHookStatus::Error,
        clankers_agent::AgentHookStatus::Cancelled => ControllerHookStatus::Cancelled,
    }
}

fn controller_hook_error_from_agent(error: &clankers_agent::AgentHookSafeError) -> ControllerHookSafeError {
    ControllerHookSafeError {
        message: error.message.clone(),
        kind: error.kind.clone(),
    }
}

fn controller_hook_usage_from_agent(usage: &clankers_agent::AgentHookUsage) -> ControllerHookUsage {
    ControllerHookUsage {
        input_tokens: usage.input_tokens,
        output_tokens: usage.output_tokens,
        cache_creation_input_tokens: usage.cache_creation_input_tokens,
        cache_read_input_tokens: usage.cache_read_input_tokens,
    }
}

impl SessionController {
    /// Compatibility constructor for daemon mode.
    ///
    /// Root/daemon assembly still passes a concrete `Agent` here, but reusable
    /// command policy constructs `AgentBackedRuntimeAdapter` explicitly before
    /// invoking concrete prompt/control operations. The remaining convergence
    /// condition is to inject the production adapter from root/daemon assembly
    /// and remove controller ownership of `Agent`.
    pub fn new(mut agent: Agent, config: ControllerConfig) -> Self {
        agent.set_session_id(config.session_id.clone());
        agent.apply_controller_thinking_level(Self::provider_thinking_level(config.initial_thinking_level));
        if let Some(service) = config.hook_service.as_ref() {
            agent = agent.with_hook_service(Arc::new(ControllerAgentHookService::new(Arc::clone(service))));
        }
        let event_rx = agent.subscribe();
        let model = config.model.clone();
        let metrics = metrics_capture::MetricsCollector::new(config.session_id.clone());

        Self {
            agent: Some(agent),
            event_rx: Some(event_rx),
            session_ledger: config.session_ledger,
            core_state: CoreState {
                thinking_level: config.initial_thinking_level,
                auto_test_enabled: config.auto_test_enabled,
                auto_test_command: config.auto_test_command.clone(),
                ..CoreState::default()
            },
            loop_engine: LoopEngine::new(),
            active_loop_id: None,
            loop_turn_output: String::new(),
            hook_service: config.hook_service,
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
            persistence_service: config.persistence_service,
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
            session_ledger: config.session_ledger,
            core_state: CoreState {
                thinking_level: config.initial_thinking_level,
                auto_test_enabled: config.auto_test_enabled,
                auto_test_command: config.auto_test_command.clone(),
                ..CoreState::default()
            },
            loop_engine: LoopEngine::new(),
            active_loop_id: None,
            loop_turn_output: String::new(),
            hook_service: config.hook_service,
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
            persistence_service: config.persistence_service,
        }
    }

    /// Set the tool rebuilder for hot-reloading tools on toggle.
    pub fn set_tool_rebuilder(&mut self, rebuilder: Arc<dyn ToolRebuilder>) {
        self.tool_rebuilder = Some(rebuilder);
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

    /// Return a cancellation handle for the current agent turn, if one exists.
    pub fn current_cancel_token(&self) -> Option<tokio_util::sync::CancellationToken> {
        self.agent.as_ref().map(|agent| agent.cancel_token())
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
        // Flush any unsaved messages to the session ledger before shutting down.
        // This catches in-progress turns that haven't hit AgentEnd yet.
        let flushed = self.flush_agent_messages_on_shutdown();
        if flushed > 0 {
            info!("session {}: flushed {flushed} unsaved messages on shutdown", self.session_id);
        }
        if let Some(ref service) = self.hook_service {
            debug!("firing SessionEnd hook");
            let payload = ControllerHookPayload::session("session-end", &self.session_id);
            let _ = service.fire(ControllerHookPoint::SessionEnd, &payload).await;
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
    pub fn metrics_summary(&self) -> &clanker_message::metrics::SessionMetricsSummary {
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

    #[derive(Clone)]
    pub struct ProviderModelServiceAdapter {
        provider: Arc<dyn clankers_provider::Provider>,
    }

    impl ProviderModelServiceAdapter {
        pub fn new(provider: Arc<dyn clankers_provider::Provider>) -> Self {
            Self { provider }
        }
    }

    pub fn model_service(provider: Arc<dyn clankers_provider::Provider>) -> Arc<dyn clankers_agent::AgentModelService> {
        Arc::new(ProviderModelServiceAdapter::new(provider))
    }

    #[async_trait::async_trait]
    impl clankers_agent::AgentModelService for ProviderModelServiceAdapter {
        async fn complete(
            &self,
            request: clankers_agent::AgentCompletionRequest,
            tx: tokio::sync::mpsc::Sender<clanker_message::streaming::StreamEvent>,
        ) -> clankers_agent::AgentModelResult<()> {
            self.provider
                .complete(provider_request_from_agent(request), tx)
                .await
                .map_err(agent_model_error_from_provider)
        }

        fn name(&self) -> &str {
            self.provider.name()
        }

        fn max_input_tokens(&self, model: &str) -> Option<usize> {
            self.provider.models().iter().find(|candidate| candidate.id == model).map(|candidate| candidate.max_input_tokens)
        }

        async fn reload_credentials(&self) {
            self.provider.reload_credentials().await;
        }
    }

    fn provider_request_from_agent(request: clankers_agent::AgentCompletionRequest) -> clankers_provider::CompletionRequest {
        clankers_provider::CompletionRequest {
            model: request.model,
            messages: request.messages,
            system_prompt: request.system_prompt,
            max_tokens: request.max_tokens,
            temperature: request.temperature,
            tools: request.tools,
            thinking: request.thinking,
            no_cache: request.no_cache,
            cache_ttl: request.cache_ttl,
            extra_params: request.extra_params,
        }
    }

    fn agent_model_error_from_provider(error: clankers_provider::error::ProviderError) -> clankers_agent::AgentModelError {
        let retryable = error.is_retryable();
        let should_compress = error.should_compress();
        let status = error.status;
        clankers_agent::AgentModelError::new(error.message)
            .with_status(status)
            .retryable(retryable)
            .should_compress(should_compress)
    }

    /// Minimal mock provider for controller tests (no actual LLM calls).
    pub struct MockProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for MockProvider {
        async fn complete(
            &self,
            _request: clankers_provider::CompletionRequest,
            _tx: tokio::sync::mpsc::Sender<clanker_message::streaming::StreamEvent>,
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
        let provider = model_service(Arc::new(MockProvider));
        let agent = Agent::new_with_agent_settings(
            provider,
            vec![],
            clankers_agent::AgentSettings::default(),
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
