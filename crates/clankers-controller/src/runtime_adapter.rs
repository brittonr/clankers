//! Runtime/session service adapter contracts for controller shell tests.
//!
//! These contracts let the controller exercise command lifecycle and semantic
//! event projection without constructing providers, databases, TUI state,
//! sockets, or desktop session storage. Production daemon mode can continue to
//! own an `Agent`; controller fixtures and future runtime-backed shells can use
//! this narrower adapter seam.

use std::sync::Arc;

use clanker_message::Content;
use clanker_message::SemanticEvent;
use clanker_message::SemanticEventMetadata;
use clanker_message::SemanticStopReason;
use clanker_message::transcript::AgentMessage;
use clankers_agent::Agent;
use clankers_agent::AgentError;
use clankers_agent::Tool;
use clankers_agent::events::AgentEvent;
use clankers_core::CoreThinkingLevel;
use tokio::sync::broadcast;
use tracing::warn;

/// Prompt request passed from the controller to a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePromptRequest {
    pub session_id: String,
    pub model: String,
    pub text: String,
    pub image_count: u32,
}

/// Prompt completion state returned by a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePromptCompletion {
    Succeeded,
    Cancelled,
    Failed { message: String },
}

impl RuntimePromptCompletion {
    fn from_agent_result(result: Result<(), AgentError>) -> Self {
        match result {
            Ok(()) => Self::Succeeded,
            Err(AgentError::Cancelled) => Self::Cancelled,
            Err(error) => Self::Failed {
                message: error.to_string(),
            },
        }
    }
}

/// Prompt result returned by a runtime/session adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePromptResult {
    pub completion: RuntimePromptCompletion,
    pub semantic_events: Vec<SemanticEvent>,
}

impl RuntimePromptResult {
    #[must_use]
    pub fn succeeded(semantic_events: Vec<SemanticEvent>) -> Self {
        Self {
            completion: RuntimePromptCompletion::Succeeded,
            semantic_events,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>, semantic_events: Vec<SemanticEvent>) -> Self {
        Self {
            completion: RuntimePromptCompletion::Failed {
                message: message.into(),
            },
            semantic_events,
        }
    }
}

/// Control requests passed from the controller to a runtime/session adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimeControlRequest {
    Abort,
    ResetCancel,
    SetThinkingLevel { level: CoreThinkingLevel },
    SetDisabledTools { tools: Vec<String> },
}

/// Control result returned by a runtime/session adapter.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RuntimeControlResult {
    pub semantic_events: Vec<SemanticEvent>,
}

/// Controller-facing runtime/session service seam.
pub trait ControllerRuntimeAdapter {
    fn submit_prompt(&mut self, request: RuntimePromptRequest) -> RuntimePromptResult;
    fn apply_control(&mut self, request: RuntimeControlRequest) -> RuntimeControlResult;
}

/// Agent-backed prompt request used by the production adapter owner.
pub struct AgentRuntimePromptRequest {
    pub session_id: String,
    pub model: String,
    pub text: String,
    pub image_content: Option<Vec<Content>>,
}

impl AgentRuntimePromptRequest {
    #[must_use]
    pub fn text_only(session_id: impl Into<String>, model: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            model: model.into(),
            text: text.into(),
            image_content: None,
        }
    }

    #[must_use]
    pub fn with_images(
        session_id: impl Into<String>,
        model: impl Into<String>,
        text: impl Into<String>,
        image_content: Vec<Content>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            model: model.into(),
            text: text.into(),
            image_content: Some(image_content),
        }
    }

    #[must_use]
    pub fn image_count(&self) -> u32 {
        self.image_content
            .as_ref()
            .map(|images| u32::try_from(images.len()).unwrap_or(u32::MAX))
            .unwrap_or_default()
    }
}

/// Production adapter owner for concrete [`Agent`] prompt and control operations.
///
/// The controller command path can use this narrow owner to keep direct agent
/// mutations out of reusable command policy while preserving the existing agent
/// event stream for metrics, persistence, hooks, and daemon projection.
pub struct AgentBackedRuntimeAdapter<'agent> {
    agent: &'agent mut Agent,
    event_rx: Option<&'agent mut broadcast::Receiver<AgentEvent>>,
}

impl<'agent> AgentBackedRuntimeAdapter<'agent> {
    #[must_use]
    pub fn new(agent: &'agent mut Agent, event_rx: Option<&'agent mut broadcast::Receiver<AgentEvent>>) -> Self {
        Self { agent, event_rx }
    }

    #[must_use]
    pub fn without_events(agent: &'agent mut Agent) -> Self {
        Self { agent, event_rx: None }
    }

    pub async fn submit_prompt(&mut self, request: AgentRuntimePromptRequest) -> RuntimePromptResult {
        self.submit_prompt_inner(request, None).await
    }

    pub async fn submit_prompt_with_event_sink(
        &mut self,
        request: AgentRuntimePromptRequest,
        on_agent_event: &mut (dyn FnMut(&AgentEvent) + Send),
    ) -> RuntimePromptResult {
        self.submit_prompt_inner(request, Some(on_agent_event)).await
    }

    async fn submit_prompt_inner(
        &mut self,
        request: AgentRuntimePromptRequest,
        mut on_agent_event: Option<&mut (dyn FnMut(&AgentEvent) + Send)>,
    ) -> RuntimePromptResult {
        let AgentRuntimePromptRequest {
            session_id,
            model,
            text,
            image_content,
        } = request;
        self.agent.set_session_id(session_id);
        debug_assert_eq!(self.agent.model(), model, "runtime prompt request model must match the agent model");
        let mut semantic_events = Vec::new();
        let agent = &mut *self.agent;
        let prompt_future = async {
            if let Some(image_content) = image_content {
                agent.prompt_with_images(&text, image_content).await
            } else {
                agent.prompt(&text).await
            }
        };
        tokio::pin!(prompt_future);

        let result = if let Some(rx) = self.event_rx.as_deref_mut() {
            loop {
                tokio::select! {
                    result = &mut prompt_future => break result,
                    event = rx.recv() => {
                        match event {
                            Ok(event) => record_agent_event(event, &mut semantic_events, &mut on_agent_event),
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("agent runtime adapter event stream lagged while prompt was running, skipped {n} events");
                            }
                            Err(broadcast::error::RecvError::Closed) => break (&mut prompt_future).await,
                        }
                    }
                }
            }
        } else {
            prompt_future.await
        };

        drain_ready_agent_events(&mut self.event_rx, &mut semantic_events, &mut on_agent_event);
        RuntimePromptResult {
            completion: RuntimePromptCompletion::from_agent_result(result),
            semantic_events,
        }
    }

    pub fn apply_runtime_control(&mut self, request: RuntimeControlRequest) -> RuntimeControlResult {
        match request {
            RuntimeControlRequest::Abort => self.abort(),
            RuntimeControlRequest::ResetCancel => self.reset_cancel(),
            RuntimeControlRequest::SetThinkingLevel { level } => self.apply_controller_thinking_level(level),
            RuntimeControlRequest::SetDisabledTools { .. } => {
                // The controller/tool rebuilder owns name → concrete-tool projection;
                // apply_core_filtered_tools() applies that rebuilt inventory after projection.
            }
        }
        RuntimeControlResult::default()
    }

    pub fn abort(&mut self) {
        self.agent.abort();
    }

    pub fn reset_cancel(&mut self) {
        self.agent.reset_cancel();
    }

    pub fn clear_messages(&mut self) {
        self.agent.clear_messages();
    }

    pub fn truncate_messages(&mut self, count: usize) {
        self.agent.truncate_messages(count);
    }

    pub fn seed_messages(&mut self, messages: Vec<AgentMessage>) {
        self.agent.seed_messages(messages);
    }

    pub fn set_system_prompt(&mut self, prompt: String) {
        self.agent.set_system_prompt(prompt);
    }

    #[must_use]
    pub fn system_prompt(&self) -> &str {
        self.agent.system_prompt()
    }

    pub fn pop_last_exchange(&mut self) {
        self.agent.pop_last_exchange();
    }

    pub fn compact_messages(&mut self) -> clankers_agent::compaction::CompactionResult {
        self.agent.compact_messages()
    }

    pub fn set_user_tool_filter(&mut self, capabilities: Option<Vec<String>>) {
        self.agent.set_user_tool_filter(capabilities);
    }

    pub fn apply_controller_thinking_level(&mut self, level: CoreThinkingLevel) {
        self.agent.apply_controller_thinking_level(provider_thinking_level(level));
    }

    pub fn apply_core_filtered_tools(&mut self, tools: Vec<Arc<dyn Tool>>) {
        self.agent.apply_core_filtered_tools(tools);
    }

    pub fn try_set_model(&mut self, model: String) -> Result<(), AgentError> {
        self.agent.try_set_model(model)
    }

    pub fn check_session_manage_authorization(&self, action: &str) -> Result<(), AgentError> {
        self.agent.check_session_manage_authorization(action)
    }

    pub fn check_prompt_authorization(&self, text: &str) -> Result<(), AgentError> {
        self.agent.check_prompt_authorization(text)
    }

    #[must_use]
    pub fn messages(&self) -> &[AgentMessage] {
        self.agent.messages()
    }
}

fn record_agent_event(
    event: AgentEvent,
    semantic_events: &mut Vec<SemanticEvent>,
    on_agent_event: &mut Option<&mut (dyn FnMut(&AgentEvent) + Send)>,
) {
    if let Some(callback) = on_agent_event.as_deref_mut() {
        callback(&event);
    }
    if let Some(semantic_event) = event.to_semantic_event() {
        semantic_events.push(semantic_event);
    }
}

fn drain_ready_agent_events(
    event_rx: &mut Option<&mut broadcast::Receiver<AgentEvent>>,
    semantic_events: &mut Vec<SemanticEvent>,
    on_agent_event: &mut Option<&mut (dyn FnMut(&AgentEvent) + Send)>,
) {
    let Some(rx) = event_rx.as_deref_mut() else {
        return;
    };

    loop {
        match rx.try_recv() {
            Ok(event) => record_agent_event(event, semantic_events, on_agent_event),
            Err(broadcast::error::TryRecvError::Lagged(n)) => {
                warn!("agent runtime adapter event drain lagged, skipped {n} events");
            }
            Err(broadcast::error::TryRecvError::Empty | broadcast::error::TryRecvError::Closed) => break,
        }
    }
}

fn provider_thinking_level(level: CoreThinkingLevel) -> clanker_message::ThinkingLevel {
    match level {
        CoreThinkingLevel::Off => clanker_message::ThinkingLevel::Off,
        CoreThinkingLevel::Low => clanker_message::ThinkingLevel::Low,
        CoreThinkingLevel::Medium => clanker_message::ThinkingLevel::Medium,
        CoreThinkingLevel::High => clanker_message::ThinkingLevel::High,
        CoreThinkingLevel::Max => clanker_message::ThinkingLevel::Max,
    }
}

/// Deterministic adapter for controller fixtures.
#[derive(Debug, Default)]
pub struct FakeRuntimeAdapter {
    prompt_results: Vec<RuntimePromptResult>,
    pub prompts: Vec<RuntimePromptRequest>,
    pub controls: Vec<RuntimeControlRequest>,
}

impl FakeRuntimeAdapter {
    #[must_use]
    pub fn new(prompt_results: Vec<RuntimePromptResult>) -> Self {
        Self {
            prompt_results,
            prompts: Vec::new(),
            controls: Vec::new(),
        }
    }
}

impl ControllerRuntimeAdapter for FakeRuntimeAdapter {
    fn submit_prompt(&mut self, request: RuntimePromptRequest) -> RuntimePromptResult {
        self.prompts.push(request);
        if self.prompt_results.is_empty() {
            return RuntimePromptResult::succeeded(vec![SemanticEvent::Completed {
                stop_reason: SemanticStopReason::Complete,
                metadata: SemanticEventMetadata::empty().with("source", "fake_runtime_adapter"),
            }]);
        }
        self.prompt_results.remove(0)
    }

    fn apply_control(&mut self, request: RuntimeControlRequest) -> RuntimeControlResult {
        self.controls.push(request);
        RuntimeControlResult::default()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clanker_message::ContentDelta;
    use clanker_message::SemanticEvent;
    use clanker_message::Usage;
    use clanker_message::streaming::MessageMetadata;
    use clanker_message::streaming::StreamEvent;
    use tokio::sync::mpsc;

    use super::*;
    use crate::test_helpers::model_service;

    struct StreamingProvider;

    #[async_trait::async_trait]
    impl clankers_provider::Provider for StreamingProvider {
        async fn complete(
            &self,
            request: clankers_provider::CompletionRequest,
            tx: mpsc::Sender<StreamEvent>,
        ) -> clankers_provider::error::Result<()> {
            tx.send(StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-agent-runtime".to_string(),
                    model: request.model,
                    role: "assistant".to_string(),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::Text { text: String::new() },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta {
                    text: "adapter answer".to_string(),
                },
            })
            .await
            .ok();
            tx.send(StreamEvent::ContentBlockStop { index: 0 }).await.ok();
            tx.send(StreamEvent::MessageDelta {
                stop_reason: Some("end_turn".to_string()),
                usage: Usage::default(),
            })
            .await
            .ok();
            tx.send(StreamEvent::MessageStop).await.ok();
            Ok(())
        }

        fn models(&self) -> &[clankers_provider::Model] {
            &[]
        }

        fn name(&self) -> &str {
            "streaming-runtime-adapter"
        }
    }

    fn test_agent() -> Agent {
        Agent::new_with_agent_settings(
            model_service(Arc::new(StreamingProvider)),
            Vec::new(),
            clankers_agent::AgentSettings::default(),
            "runtime-model".to_string(),
            "You are a runtime adapter test assistant.".to_string(),
        )
    }

    #[test]
    fn fake_runtime_adapter_records_prompts_and_controls_without_desktop_services() {
        let mut adapter =
            FakeRuntimeAdapter::new(vec![RuntimePromptResult::succeeded(vec![SemanticEvent::AssistantDelta {
                text: "ok".to_string(),
                metadata: SemanticEventMetadata::empty().with("source", "fixture"),
            }])]);

        let result = adapter.submit_prompt(RuntimePromptRequest {
            session_id: "session-adapter".to_string(),
            model: "model-adapter".to_string(),
            text: "hello".to_string(),
            image_count: 0,
        });
        adapter.apply_control(RuntimeControlRequest::SetThinkingLevel {
            level: CoreThinkingLevel::High,
        });

        assert_eq!(adapter.prompts.len(), 1);
        assert_eq!(adapter.controls.len(), 1);
        assert!(matches!(result.completion, RuntimePromptCompletion::Succeeded));
        assert_eq!(result.semantic_events[0].kind(), "assistant_delta");
    }

    #[tokio::test]
    async fn agent_backed_runtime_adapter_projects_agent_prompt_events_and_completion() {
        let mut agent = test_agent();
        agent.set_session_id("runtime-session".to_string());
        let mut event_rx = agent.subscribe();
        let mut observed_agent_event_kinds = Vec::new();

        let result = {
            let mut adapter = AgentBackedRuntimeAdapter::new(&mut agent, Some(&mut event_rx));
            let mut record_event = |event: &AgentEvent| {
                observed_agent_event_kinds.push(event.event_kind().to_string());
            };
            adapter
                .submit_prompt_with_event_sink(
                    AgentRuntimePromptRequest::text_only("runtime-session", "runtime-model", "hello adapter"),
                    &mut record_event,
                )
                .await
        };

        assert!(matches!(result.completion, RuntimePromptCompletion::Succeeded));
        let semantic_kinds = result.semantic_events.iter().map(SemanticEvent::kind).collect::<Vec<_>>();
        assert!(semantic_kinds.contains(&"agent_start"));
        assert!(semantic_kinds.contains(&"assistant_delta"));
        assert!(semantic_kinds.contains(&"agent_end"));
        assert!(observed_agent_event_kinds.iter().any(|kind| kind == "agent_start"));
        assert!(observed_agent_event_kinds.iter().any(|kind| kind == "message_update"));
        assert_eq!(agent.messages().len(), 2);
    }

    #[test]
    fn agent_backed_runtime_adapter_wraps_control_operations() {
        let mut agent = test_agent();

        {
            let mut adapter = AgentBackedRuntimeAdapter::without_events(&mut agent);
            adapter.apply_runtime_control(RuntimeControlRequest::SetThinkingLevel {
                level: CoreThinkingLevel::High,
            });
            adapter.set_system_prompt("updated system prompt".to_string());
            adapter.apply_runtime_control(RuntimeControlRequest::Abort);
        }
        assert_eq!(agent.thinking_level(), clanker_message::ThinkingLevel::High);
        assert_eq!(agent.system_prompt(), "updated system prompt");
        assert!(agent.cancel_token().is_cancelled());

        {
            let mut adapter = AgentBackedRuntimeAdapter::without_events(&mut agent);
            adapter.apply_runtime_control(RuntimeControlRequest::ResetCancel);
        }
        assert!(!agent.cancel_token().is_cancelled());
    }
}
