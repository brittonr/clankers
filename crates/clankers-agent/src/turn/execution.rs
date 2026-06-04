//! Tool execution logic and turn execution flow

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use clanker_message::Content;
use clanker_message::ContentDelta;
#[cfg(test)]
use clanker_message::MessageMetadata;
use clanker_message::StopReason;
use clanker_message::StreamEvent;
use clanker_message::Usage;
use clanker_message::transcript::*;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineToolCall;
use clankers_engine_host::stream::HostStreamEvent;
use clankers_engine_host::stream::ProviderStreamError;
use clankers_provider::CompletionRequest;
use clankers_provider::Provider;
use clankers_tool_host::CapabilityDecision;
use clankers_tool_host::ToolCancellationService;
use clankers_tool_host::ToolCapabilityRequest;
use clankers_tool_host::ToolCapabilityService;
use clankers_tool_host::ToolCatalog;
use clankers_tool_host::ToolDescriptor;
use clankers_tool_host::ToolExecutor;
use clankers_tool_host::ToolHookDecision;
use clankers_tool_host::ToolHookPhase;
use clankers_tool_host::ToolHookRequest;
use clankers_tool_host::ToolHookService;
use clankers_tool_host::ToolHostError;
use clankers_tool_host::ToolHostFuture;
use clankers_tool_host::ToolHostOutcome;
use clankers_tool_host::ToolInvocationCancellation;
use clankers_tool_host::ToolProgressEvent;
use clankers_tool_host::ToolProgressSink;
use clankers_tool_host::ToolSearchHit;
use clankers_tool_host::ToolSearchRequest;
use clankers_tool_host::ToolSearchResult;
use clankers_tool_host::ToolSearchService;
use serde_json::Value;
use tokio::sync::broadcast;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::CollectedResponse;
use super::ContentBlockBuilder;
use super::ControllerToolServices;
use super::ports::AgentToolEventSink;
use super::ports::LegacyToolRunner;
use super::steel_tool_substrate::AgentToolSteelSubstrateConfig;
use super::steel_tool_substrate::authorize_tool_invocation;
use super::steel_tool_substrate::blocked_receipt_to_tool_result;
use crate::error::AgentError;
use crate::error::Result;
use crate::events::AgentEvent;
use crate::tool::Tool;
use crate::tool::ToolContext;
use crate::tool::ToolResult as ToolExecResult;
use crate::tool::progress::ToolResultAccumulator;

const ENGINE_REQUEST_ASSISTANT_MODEL: &str = "engine-assistant";
const ENGINE_REQUEST_TOOL_NAME: &str = "engine-tool";

pub(super) struct ProviderStreamNormalizer {
    model: Option<String>,
    stop_reason: StopReason,
}

impl ProviderStreamNormalizer {
    #[must_use]
    pub(super) fn new() -> Self {
        Self {
            model: None,
            stop_reason: StopReason::Stop,
        }
    }

    pub(super) fn push(&mut self, event: StreamEvent) -> Vec<HostStreamEvent> {
        match event {
            StreamEvent::MessageStart { message } => {
                self.model = Some(message.model);
                Vec::new()
            }
            StreamEvent::ContentBlockStart { index, content_block } => {
                host_events_from_content_block_start(index, content_block)
            }
            StreamEvent::ContentBlockDelta { index, delta } => host_events_from_content_delta(index, delta),
            StreamEvent::ContentBlockStop { index } => vec![HostStreamEvent::ContentBlockStop { index }],
            StreamEvent::MessageDelta { stop_reason, usage } => {
                if let Some(reason) = stop_reason {
                    self.stop_reason = super::parse_stop_reason(&reason);
                }
                vec![HostStreamEvent::Usage { usage }]
            }
            StreamEvent::MessageStop => vec![HostStreamEvent::MessageStop {
                model: self.model.clone(),
                stop_reason: self.stop_reason.clone(),
            }],
            StreamEvent::Error { error } => vec![HostStreamEvent::ProviderError {
                error: ProviderStreamError {
                    message: error,
                    status: None,
                    retryable: false,
                },
            }],
        }
    }
}

fn host_events_from_content_block_start(index: usize, content_block: Content) -> Vec<HostStreamEvent> {
    match content_block {
        Content::Text { text } => {
            let mut events = vec![HostStreamEvent::TextStart { index }];
            if !text.is_empty() {
                events.push(HostStreamEvent::TextDelta { index, text });
            }
            events
        }
        Content::Thinking { thinking, signature } => {
            let mut events = vec![HostStreamEvent::ThinkingStart { index, signature }];
            if !thinking.is_empty() {
                events.push(HostStreamEvent::ThinkingDelta { index, thinking });
            }
            events
        }
        Content::ToolUse { id, name, input } => {
            let mut events = vec![HostStreamEvent::ToolUseStart { index, id, name }];
            if input.is_object() && input.as_object().is_some_and(|object| !object.is_empty()) {
                events.push(HostStreamEvent::ToolUseJsonDelta {
                    index,
                    json: input.to_string(),
                });
            }
            events
        }
        Content::Image { .. } | Content::ToolResult { .. } => Vec::new(),
    }
}

fn host_events_from_content_delta(index: usize, delta: ContentDelta) -> Vec<HostStreamEvent> {
    match delta {
        ContentDelta::TextDelta { text } => vec![HostStreamEvent::TextDelta { index, text }],
        ContentDelta::ThinkingDelta { thinking } => vec![HostStreamEvent::ThinkingDelta { index, thinking }],
        ContentDelta::InputJsonDelta { partial_json } => vec![HostStreamEvent::ToolUseJsonDelta {
            index,
            json: partial_json,
        }],
        ContentDelta::SignatureDelta { .. } => Vec::new(),
    }
}

pub(super) fn tool_definitions_from_tool_catalog(
    controller_tools: &HashMap<String, Arc<dyn Tool>>,
) -> Vec<crate::tool::ToolDefinition> {
    AgentToolCatalog { controller_tools }.tool_definitions()
}

struct AgentToolCatalog<'a> {
    controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
}

impl AgentToolCatalog<'_> {
    fn tool_definitions(&self) -> Vec<crate::tool::ToolDefinition> {
        self.controller_tools.values().map(|tool| tool.definition().clone()).collect()
    }
}

impl ToolCatalog for AgentToolCatalog<'_> {
    fn describe_tools(&self) -> Vec<ToolDescriptor> {
        self.controller_tools
            .values()
            .map(|tool| {
                let definition = tool.definition();
                ToolDescriptor {
                    name: definition.name.clone(),
                    description: definition.description.clone(),
                }
            })
            .collect()
    }

    fn contains_tool(&self, name: &str) -> bool {
        self.controller_tools.contains_key(name)
    }
}

impl ControllerToolServices {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_concrete(
        event_tx: broadcast::Sender<AgentEvent>,
        cancel: CancellationToken,
        hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
        session_id: String,
        db: Option<clankers_db::Db>,
        capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
        user_tool_filter: Option<Vec<String>>,
        steel_tool_substrate: Option<AgentToolSteelSubstrateConfig>,
    ) -> Self {
        let events = Arc::new(BroadcastAgentToolEventSink {
            event_tx: event_tx.clone(),
        });
        let progress = Arc::new(AgentEventProgressSink {
            event_tx: event_tx.clone(),
        });
        let cancellation = Arc::new(TokenToolCancellationService { cancel: cancel.clone() });
        let hooks = hook_pipeline.clone().map(|pipeline| {
            Arc::new(ControllerHookService {
                pipeline,
                session_id: session_id.clone(),
            }) as Arc<dyn ToolHookService>
        });
        let capability = (capability_gate.is_some() || user_tool_filter.is_some()).then(|| {
            Arc::new(ControllerCapabilityService {
                capability_gate,
                user_tool_filter,
            }) as Arc<dyn ToolCapabilityService>
        });
        let search = db.clone().map(|db| Arc::new(DbMemorySearchService { db }) as Arc<dyn ToolSearchService>);
        let legacy_runner = Arc::new(AgentLegacyToolRunner {
            event_tx,
            cancel,
            hook_pipeline,
            session_id,
            db,
        });
        Self {
            events,
            progress,
            cancellation,
            storage: None,
            search,
            hooks,
            capability,
            legacy_runner,
            steel_tool_substrate,
        }
    }
}

struct BroadcastAgentToolEventSink {
    event_tx: broadcast::Sender<AgentEvent>,
}

impl AgentToolEventSink for BroadcastAgentToolEventSink {
    fn emit(&self, event: AgentEvent) {
        self.event_tx.send(event).ok();
    }
}

struct AgentEventProgressSink {
    event_tx: broadcast::Sender<AgentEvent>,
}

impl ToolProgressSink for AgentEventProgressSink {
    fn emit(&self, event: ToolProgressEvent) -> std::result::Result<(), ToolHostError> {
        let result = ToolExecResult::text(event.message);
        self.event_tx
            .send(AgentEvent::ToolExecutionUpdate {
                call_id: event.call_id,
                partial: result,
            })
            .ok();
        Ok(())
    }
}

struct TokenToolCancellationService {
    cancel: CancellationToken,
}

impl ToolCancellationService for TokenToolCancellationService {
    fn cancellation_state(&self, _call_id: &str) -> ToolInvocationCancellation {
        if self.cancel.is_cancelled() {
            return ToolInvocationCancellation::cancelled("cancelled");
        }
        ToolInvocationCancellation::active()
    }
}

struct DbMemorySearchService {
    db: clankers_db::Db,
}

impl ToolSearchService for DbMemorySearchService {
    fn search(&self, request: ToolSearchRequest) -> std::result::Result<ToolSearchResult, ToolHostError> {
        let scope_filter = request.metadata.get("scope").map(String::as_str).unwrap_or("all");
        let entries = self.db.memory().search(&request.query).map_err(|error| ToolHostError::HostFailed {
            message: error.to_string(),
        })?;
        let hits = entries
            .into_iter()
            .filter(|entry| match scope_filter {
                "global" => matches!(entry.scope, clankers_db::memory::MemoryScope::Global),
                "project" => matches!(entry.scope, clankers_db::memory::MemoryScope::Project { .. }),
                _ => true,
            })
            .take(usize::try_from(request.limit).unwrap_or(usize::MAX))
            .enumerate()
            .map(|(index, entry)| {
                let mut metadata = BTreeMap::new();
                metadata.insert("memory_id".to_string(), entry.id.to_string());
                metadata.insert("scope".to_string(), entry.scope.to_string());
                ToolSearchHit {
                    title: entry.id.to_string(),
                    snippet: entry.text,
                    rank: u32::try_from(index + 1).unwrap_or(u32::MAX),
                    metadata,
                }
            })
            .collect();
        Ok(ToolSearchResult { hits })
    }
}

struct ControllerCapabilityService {
    capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
    user_tool_filter: Option<Vec<String>>,
}

impl ToolCapabilityService for ControllerCapabilityService {
    fn check(&self, request: ToolCapabilityRequest) -> std::result::Result<CapabilityDecision, ToolHostError> {
        if let Some(gate) = &self.capability_gate
            && let Err(reason) = gate.check_tool_call(&request.tool_name, &request.input)
        {
            return Ok(CapabilityDecision::Denied { reason });
        }

        if let Some(filter) = &self.user_tool_filter {
            let allowed = filter.iter().any(|pattern| {
                pattern == "*" || pattern.split(',').any(|candidate| candidate.trim() == request.tool_name)
            });
            if !allowed {
                return Ok(CapabilityDecision::Denied {
                    reason: "Tool not in active capability set (use /capabilities to adjust)".to_string(),
                });
            }
        }

        Ok(CapabilityDecision::Allowed)
    }
}

struct ControllerHookService {
    pipeline: Arc<clankers_hooks::HookPipeline>,
    session_id: String,
}

impl ToolHookService for ControllerHookService {
    fn decide(
        &self,
        request: ToolHookRequest,
    ) -> ToolHostFuture<'_, std::result::Result<ToolHookDecision, ToolHostError>> {
        let pipeline = self.pipeline.clone();
        let session_id = self.session_id.clone();
        Box::pin(async move {
            let hook_point = match request.phase {
                ToolHookPhase::Before => clankers_hooks::HookPoint::PreTool,
                ToolHookPhase::After => clankers_hooks::HookPoint::PostTool,
            };
            let event_name = match request.phase {
                ToolHookPhase::Before => "pre-tool",
                ToolHookPhase::After => "post-tool",
            };
            let result_json = (request.phase == ToolHookPhase::After).then(|| request.input.clone());
            let input = if request.phase == ToolHookPhase::After {
                serde_json::json!({})
            } else {
                request.input.clone()
            };
            let payload = clankers_hooks::HookPayload::tool(
                event_name,
                &session_id,
                &request.tool_name,
                &request.call_id,
                input,
                result_json,
            );
            let decision = pipeline.fire(hook_point, &payload).await;
            Ok(match decision {
                clankers_hooks::HookVerdict::Continue => ToolHookDecision::Continue,
                clankers_hooks::HookVerdict::Modify(input) => ToolHookDecision::Modify { input },
                clankers_hooks::HookVerdict::Deny { reason } => ToolHookDecision::Deny { reason },
            })
        })
    }
}

struct AgentLegacyToolRunner {
    event_tx: broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    session_id: String,
    db: Option<clankers_db::Db>,
}

#[async_trait]
impl LegacyToolRunner for AgentLegacyToolRunner {
    async fn execute_legacy_tool(&self, tool: Arc<dyn Tool>, call_id: String, input: Value) -> ToolExecResult {
        execute_tool_with_accumulator(
            tool,
            &call_id,
            input,
            &self.event_tx,
            self.cancel.clone(),
            self.hook_pipeline.clone(),
            self.session_id.clone(),
            self.db.clone(),
        )
        .await
    }
}

#[derive(Clone)]
struct AgentSingleToolExecutor {
    tool: Option<Arc<dyn Tool>>,
    services: ControllerToolServices,
}

impl ToolExecutor for AgentSingleToolExecutor {
    async fn execute_tool(&mut self, call: EngineToolCall) -> ToolHostOutcome {
        let message = execute_single_tool(
            self.tool.clone(),
            call.call_id.0.clone(),
            call.tool_name.clone(),
            call.input,
            self.services.clone(),
        )
        .await;
        tool_result_message_to_host_outcome(&message)
    }
}

pub(super) fn tool_result_message_to_host_outcome(message: &ToolResultMessage) -> ToolHostOutcome {
    let details = message.details.clone().unwrap_or(Value::Null);
    if message.is_error {
        return ToolHostOutcome::ToolError {
            content: message.content.clone(),
            details,
            message: first_text_block(&message.content).unwrap_or_else(|| "tool execution failed".to_string()),
        };
    }

    ToolHostOutcome::Succeeded {
        content: message.content.clone(),
        details,
    }
}

fn tool_result_message_from_host_outcome(
    call_id: String,
    tool_name: String,
    outcome: ToolHostOutcome,
) -> ToolResultMessage {
    match outcome {
        ToolHostOutcome::Succeeded { content, details } => {
            tool_result_message(call_id, tool_name, content, false, details)
        }
        ToolHostOutcome::ToolError {
            content,
            details,
            message,
        } => {
            let content = if content.is_empty() {
                vec![Content::Text { text: message }]
            } else {
                content
            };
            tool_result_message(call_id, tool_name, content, true, details)
        }
        ToolHostOutcome::MissingTool { name } => {
            tool_result_error(call_id, tool_name, format!("Tool '{name}' not found"))
        }
        ToolHostOutcome::CapabilityDenied { name, reason } => {
            tool_result_error(call_id, tool_name, format!("🔒 {name}: {reason}"))
        }
        ToolHostOutcome::Cancelled { name } => tool_result_error(call_id, tool_name, format!("tool cancelled: {name}")),
        ToolHostOutcome::Truncated { content, metadata } => {
            tool_result_message(call_id, tool_name, content, false, serde_json::json!({ "truncation": metadata }))
        }
    }
}

fn tool_result_message(
    call_id: String,
    tool_name: String,
    content: Vec<Content>,
    is_error: bool,
    details: Value,
) -> ToolResultMessage {
    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content,
        is_error,
        details: Some(details).filter(|details| !details.is_null()),
        timestamp: Utc::now(),
    }
}

fn tool_result_error(call_id: String, tool_name: String, message: String) -> ToolResultMessage {
    tool_result_message(call_id, tool_name, vec![Content::Text { text: message }], true, Value::Null)
}

fn first_text_block(content: &[Content]) -> Option<String> {
    content.iter().find_map(|block| match block {
        Content::Text { text } => Some(text.clone()),
        Content::Image { .. } | Content::Thinking { .. } | Content::ToolUse { .. } | Content::ToolResult { .. } => None,
    })
}

/// Execute a single engine-requested model call: stream response and collect results.
pub(super) async fn stream_model_request(
    provider: &dyn Provider,
    request: CompletionRequest,
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: &CancellationToken,
) -> Result<CollectedResponse> {
    let (stream_tx, mut stream_rx) = mpsc::channel(256);
    let event_tx_clone = event_tx.clone();
    let complete_fut = provider.complete(request, stream_tx);
    let collect_fut = collect_stream_events(&mut stream_rx, &event_tx_clone);

    let (complete_result, collected) = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            return Err(AgentError::Cancelled);
        }
        result = async { tokio::join!(complete_fut, collect_fut) } => result,
    };
    complete_result?;
    collected
}

pub(super) fn engine_messages_from_agent_messages(messages: &[AgentMessage]) -> Vec<EngineMessage> {
    messages
        .iter()
        .filter_map(|message| match message {
            AgentMessage::User(user) => Some(EngineMessage {
                role: EngineMessageRole::User,
                content: user.content.clone(),
            }),
            AgentMessage::Assistant(assistant) => Some(EngineMessage {
                role: EngineMessageRole::Assistant,
                content: assistant.content.clone(),
            }),
            AgentMessage::ToolResult(tool_result) => Some(EngineMessage {
                role: EngineMessageRole::Tool,
                content: vec![Content::ToolResult {
                    tool_use_id: tool_result.call_id.clone(),
                    content: tool_result.content.clone(),
                    is_error: if tool_result.is_error { Some(true) } else { None },
                }],
            }),
            AgentMessage::BashExecution(_)
            | AgentMessage::Custom(_)
            | AgentMessage::BranchSummary(_)
            | AgentMessage::CompactionSummary(_) => None,
        })
        .collect()
}

pub(super) fn completion_request_from_engine_request(engine_request: &EngineModelRequest) -> Result<CompletionRequest> {
    let messages = agent_messages_from_engine_messages(&engine_request.messages)?;
    Ok(CompletionRequest {
        model: engine_request.model.clone(),
        messages,
        system_prompt: Some(engine_request.system_prompt.clone()),
        max_tokens: engine_request.max_tokens,
        temperature: engine_request.temperature,
        tools: engine_request.tools.clone(),
        thinking: engine_request.thinking.clone(),
        no_cache: engine_request.no_cache,
        cache_ttl: engine_request.cache_ttl.clone(),
        extra_params: build_extra_params(&engine_request.session_id),
    })
}

fn agent_messages_from_engine_messages(messages: &[EngineMessage]) -> Result<Vec<AgentMessage>> {
    let request_timestamp = Utc::now();
    let mut converted_messages = Vec::with_capacity(messages.len());

    for message in messages {
        let agent_message = match message.role {
            EngineMessageRole::User => AgentMessage::User(UserMessage {
                id: MessageId::generate(),
                content: message.content.clone(),
                timestamp: request_timestamp,
            }),
            EngineMessageRole::Assistant => AgentMessage::Assistant(AssistantMessage {
                id: MessageId::generate(),
                content: message.content.clone(),
                model: ENGINE_REQUEST_ASSISTANT_MODEL.to_string(),
                usage: Usage::default(),
                stop_reason: StopReason::Stop,
                timestamp: request_timestamp,
            }),
            EngineMessageRole::Tool => {
                AgentMessage::ToolResult(tool_result_message_from_engine_message(message, request_timestamp)?)
            }
        };
        converted_messages.push(agent_message);
    }

    Ok(converted_messages)
}

fn tool_result_message_from_engine_message(
    message: &EngineMessage,
    request_timestamp: chrono::DateTime<chrono::Utc>,
) -> Result<ToolResultMessage> {
    let Some(Content::ToolResult {
        tool_use_id,
        content,
        is_error,
    }) = message.content.first()
    else {
        return Err(AgentError::ProviderStreaming {
            message: "engine emitted a tool-role message without a tool_result content block".to_string(),
            status: None,
            retryable: false,
        });
    };

    Ok(ToolResultMessage {
        id: MessageId::generate(),
        call_id: tool_use_id.clone(),
        tool_name: ENGINE_REQUEST_TOOL_NAME.to_string(),
        content: content.clone(),
        is_error: is_error.unwrap_or(false),
        details: None,
        timestamp: request_timestamp,
    })
}

fn build_extra_params(session_id: &str) -> HashMap<String, Value> {
    if session_id.is_empty() {
        return HashMap::new();
    }

    HashMap::from([("_session_id".to_string(), Value::String(session_id.to_string()))])
}

/// Collect streaming events into a complete response
pub(super) async fn collect_stream_events(
    stream_rx: &mut mpsc::Receiver<StreamEvent>,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> Result<CollectedResponse> {
    let mut content_builders: Vec<ContentBlockBuilder> = Vec::new();
    let mut host_stream_normalizer = ProviderStreamNormalizer::new();
    let mut model = String::new();
    let mut usage = Usage::default();
    let mut stop_reason = StopReason::Stop;

    while let Some(event) = stream_rx.recv().await {
        let _host_stream_events = host_stream_normalizer.push(event.clone());
        match event {
            StreamEvent::MessageStart { message } => {
                model.clone_from(&message.model);
            }
            StreamEvent::ContentBlockStart { index, content_block } => {
                // Ensure we have enough slots
                while content_builders.len() <= index {
                    content_builders.push(ContentBlockBuilder::new(Content::Text { text: String::new() }));
                }
                content_builders[index] = ContentBlockBuilder::new(content_block.clone());

                // Forward to TUI/consumers
                event_tx.send(AgentEvent::ContentBlockStart { index, content_block }).ok();
            }
            StreamEvent::ContentBlockDelta { index, delta } => {
                // Forward delta event with index
                event_tx
                    .send(AgentEvent::MessageUpdate {
                        index,
                        delta: delta.clone(),
                    })
                    .ok();

                // Apply delta to content block builder
                if let Some(builder) = content_builders.get_mut(index) {
                    builder.apply_delta(&delta);
                }
            }
            StreamEvent::ContentBlockStop { index } => {
                // Forward to TUI/consumers
                event_tx.send(AgentEvent::ContentBlockStop { index }).ok();
            }
            StreamEvent::MessageDelta {
                stop_reason: sr,
                usage: u,
            } => {
                if let Some(reason) = sr {
                    stop_reason = super::parse_stop_reason(&reason);
                }
                // Update usage (keep higher values from message_delta)
                if u.output_tokens > 0 {
                    usage.output_tokens = u.output_tokens;
                }
                if u.input_tokens > 0 {
                    usage.input_tokens = u.input_tokens;
                }
                if u.cache_read_input_tokens > 0 {
                    usage.cache_read_input_tokens = u.cache_read_input_tokens;
                }
                if u.cache_creation_input_tokens > 0 {
                    usage.cache_creation_input_tokens = u.cache_creation_input_tokens;
                }
            }
            StreamEvent::MessageStop => {
                break;
            }
            StreamEvent::Error { error } => {
                return Err(AgentError::ProviderStreaming {
                    message: error,
                    status: None,
                    retryable: false,
                });
            }
        }
    }

    // Finalize all content blocks
    let content: Vec<Content> = content_builders.into_iter().map(|builder| builder.finalize()).collect();

    Ok(CollectedResponse {
        content,
        model,
        usage,
        stop_reason,
    })
}

/// Execute tools in parallel and return their results
#[cfg(test)]
pub(super) async fn execute_tools_parallel(
    controller_tools: &HashMap<String, Arc<dyn Tool>>,
    tool_calls: &[(String, String, Value)],
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    session_id: &str,
    db: Option<clankers_db::Db>,
    capability_gate: Option<Arc<dyn crate::tool::CapabilityGate>>,
    user_tool_filter: Option<Vec<String>>,
) -> Vec<ToolResultMessage> {
    let services = ControllerToolServices::from_concrete(
        event_tx.clone(),
        cancel,
        hook_pipeline,
        session_id.to_string(),
        db,
        capability_gate,
        user_tool_filter,
        None,
    );
    execute_tools_parallel_with_substrate(controller_tools, tool_calls, services).await
}

// Clippy allowance: this adapter mirrors the engine/tool-host seam while the
// per-call context is still assembled from independent controller-owned slots.
#[allow(clippy::too_many_arguments)]
pub(super) async fn execute_tools_parallel_with_substrate(
    controller_tools: &HashMap<String, Arc<dyn Tool>>,
    tool_calls: &[(String, String, Value)],
    services: ControllerToolServices,
) -> Vec<ToolResultMessage> {
    use futures::future::BoxFuture;
    use futures::future::FutureExt;

    let futures: Vec<BoxFuture<'static, ToolResultMessage>> = tool_calls
        .iter()
        .map(|(call_id, tool_name, input)| {
            let call = EngineToolCall {
                call_id: EngineCorrelationId(call_id.clone()),
                tool_name: tool_name.clone(),
                input: input.clone(),
            };
            let mut executor = AgentSingleToolExecutor {
                tool: controller_tools.get(tool_name).cloned(),
                services: services.clone(),
            };
            let call_id = call_id.clone();
            let tool_name = tool_name.clone();
            async move {
                let outcome = executor.execute_tool(call).await;
                tool_result_message_from_host_outcome(call_id, tool_name, outcome)
            }
            .boxed()
        })
        .collect();

    futures::future::join_all(futures).await
}

/// Execute a single tool and return its result message
// Clippy allowance: this boundary receives the fully expanded tool context from
// both legacy parallel execution and the engine host adapter.
#[allow(clippy::too_many_arguments)]
async fn execute_single_tool(
    tool: Option<Arc<dyn Tool>>,
    call_id: String,
    tool_name: String,
    input: Value,
    services: ControllerToolServices,
) -> ToolResultMessage {
    let events = services.events.clone();
    let mut invocation_context = services.invocation_context(&call_id);

    // Emit ToolCall event
    events.emit(AgentEvent::ToolCall {
        tool_name: tool_name.clone(),
        call_id: call_id.clone(),
        input: input.clone(),
    });

    invocation_context = invocation_context.with_cancellation(services.cancellation.cancellation_state(&call_id));
    if let Err(outcome) = invocation_context.ensure_not_cancelled(&tool_name) {
        return create_error_result_from_host_outcome(call_id, tool_name, outcome, events.as_ref());
    }

    if let Some(capability) = invocation_context.capability_service.as_ref() {
        match capability.check(ToolCapabilityRequest::new(&call_id, &tool_name, input.clone())) {
            Ok(decision) => {
                invocation_context = invocation_context.with_capability(decision);
            }
            Err(error) => {
                return create_error_result_with_event_sink(call_id, tool_name, error.to_string(), events.as_ref());
            }
        }
    }
    if let Err(outcome) = invocation_context.ensure_allowed(&tool_name) {
        return create_error_result_from_host_outcome(call_id, tool_name, outcome, events.as_ref());
    }

    // Check if tool exists
    let Some(tool) = tool else {
        let error_msg = format!("Tool '{}' not found", tool_name);
        return create_error_result_with_event_sink(call_id, tool_name, error_msg, events.as_ref());
    };

    // Check sandbox paths
    if let Some(reason) = check_tool_paths(&input) {
        return create_error_result_with_event_sink(call_id, tool_name, format!("🔒 {}", reason), events.as_ref());
    }

    // Fire pre-tool hook (can deny or modify input)
    let effective_input = if let Some(hooks) = invocation_context.hooks.as_ref() {
        match hooks.decide(ToolHookRequest::before(&call_id, &tool_name, input.clone())).await {
            Ok(ToolHookDecision::Deny { reason }) => {
                return create_error_result_with_event_sink(
                    call_id,
                    tool_name,
                    format!("🪝 Hook denied: {reason}"),
                    events.as_ref(),
                );
            }
            Ok(ToolHookDecision::Modify { input }) => input,
            Ok(ToolHookDecision::Continue) => input,
            Err(error) => {
                return create_error_result_with_event_sink(call_id, tool_name, error.to_string(), events.as_ref());
            }
        }
    } else {
        input
    };

    if let Err(receipt) = authorize_tool_invocation(
        services.steel_tool_substrate.as_ref(),
        tool.as_ref(),
        &call_id,
        &tool_name,
        &effective_input,
        |event| events.emit(event),
    ) {
        return blocked_receipt_to_tool_result(call_id, tool_name, *receipt);
    }

    events.emit(AgentEvent::ToolExecutionStart {
        call_id: call_id.clone(),
        tool_name: tool_name.clone(),
    });

    let result = if tool.uses_neutral_tool_context() {
        tool.execute_with_neutral_context(invocation_context.clone(), effective_input).await
    } else {
        // Execute with accumulator through the legacy compatibility runner.
        services.legacy_runner.execute_legacy_tool(tool, call_id.clone(), effective_input).await
    };

    fire_post_tool_hook(invocation_context.hooks.as_deref(), &tool_name, &call_id, &result).await;

    events.emit(AgentEvent::ToolExecutionEnd {
        call_id: call_id.clone(),
        result: result.clone(),
        is_error: result.is_error,
    });

    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content: super::tool_result_content_to_message_content(&result.content),
        is_error: result.is_error,
        details: result.details,
        timestamp: Utc::now(),
    }
}

async fn fire_post_tool_hook(
    hooks: Option<&dyn ToolHookService>,
    tool_name: &str,
    call_id: &str,
    result: &ToolExecResult,
) {
    let Some(hooks) = hooks else {
        return;
    };
    let result_json = serde_json::to_value(result).unwrap_or_else(|_| serde_json::json!({ "serialization": "failed" }));
    hooks.decide(ToolHookRequest::after(call_id, tool_name, result_json)).await.ok();
}

/// Execute tool with result accumulator for streaming output
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(unbounded_loop, reason = "iteration loop; bounded by tool result collection")
)]
async fn execute_tool_with_accumulator(
    tool: Arc<dyn Tool>,
    call_id: &str,
    input: Value,
    event_tx: &broadcast::Sender<AgentEvent>,
    cancel: CancellationToken,
    hook_pipeline: Option<Arc<clankers_hooks::HookPipeline>>,
    session_id: String,
    db: Option<clankers_db::Db>,
) -> ToolExecResult {
    // Subscribe to event bus BEFORE tool execution to capture all chunks
    let mut chunk_rx = event_tx.subscribe();
    let accumulator = Arc::new(parking_lot::Mutex::new(ToolResultAccumulator::new()));
    let acc_clone = accumulator.clone();
    let call_id_for_collector = call_id.to_string();

    // Spawn collector task that feeds ToolResultChunk events into accumulator
    let collector = tokio::spawn(async move {
        loop {
            match chunk_rx.recv().await {
                Ok(AgentEvent::ToolResultChunk { call_id: cid, chunk }) if cid == call_id_for_collector => {
                    acc_clone.lock().push(chunk);
                }
                Ok(_) => {} // ignore other events
                Err(broadcast::error::RecvError::Closed) => break,
                Err(broadcast::error::RecvError::Lagged(_)) => {}
            }
        }
    });

    // Execute tool
    let mut ctx =
        ToolContext::new(call_id.to_string(), cancel, Some(event_tx.clone())).with_session_id(session_id.clone());
    if let Some(pipeline) = hook_pipeline {
        ctx = ctx.with_hooks(pipeline, session_id);
    }
    if let Some(db) = db {
        ctx = ctx.with_db(db);
    }
    let direct_result = tool.execute(&ctx, input).await;

    // Stop collector and decide which result to use
    collector.abort();
    collector.await.ok();

    let acc = std::mem::take(&mut *accumulator.lock());
    if acc.total_bytes() > 0 {
        // Chunks were collected — use accumulated (truncated) result
        let mut accumulated = acc.finalize();
        // Preserve error status from the direct result
        accumulated.is_error = direct_result.is_error;
        accumulated
    } else {
        // No chunks emitted — use tool's direct return (backward compat)
        direct_result
    }
}

/// Create an error result message
pub(super) fn create_error_result(
    call_id: String,
    tool_name: String,
    error_msg: String,
    event_tx: &broadcast::Sender<AgentEvent>,
) -> ToolResultMessage {
    let sink = BroadcastAgentToolEventSink {
        event_tx: event_tx.clone(),
    };
    create_error_result_with_event_sink(call_id, tool_name, error_msg, &sink)
}

fn create_error_result_from_host_outcome(
    call_id: String,
    tool_name: String,
    outcome: ToolHostOutcome,
    events: &dyn AgentToolEventSink,
) -> ToolResultMessage {
    let message = tool_result_message_from_host_outcome(call_id.clone(), tool_name.clone(), outcome);
    let error_msg = first_text_block(&message.content).unwrap_or_else(|| "tool execution failed".to_string());
    create_error_result_with_event_sink(call_id, tool_name, error_msg, events)
}

fn create_error_result_with_event_sink(
    call_id: String,
    tool_name: String,
    error_msg: String,
    events: &dyn AgentToolEventSink,
) -> ToolResultMessage {
    let result = ToolExecResult::error(error_msg);

    events.emit(AgentEvent::ToolExecutionEnd {
        call_id: call_id.clone(),
        result: result.clone(),
        is_error: true,
    });

    ToolResultMessage {
        id: MessageId::generate(),
        call_id,
        tool_name,
        content: super::tool_result_content_to_message_content(&result.content),
        is_error: true,
        details: result.details,
        timestamp: Utc::now(),
    }
}

/// Check all path-like parameters in a tool call against the sandbox path policy.
///
/// Extracts values from common parameter names (`path`, `file`, `directory`,
/// `cwd`, `command`) and checks each against the global deny-list.
/// For `command` parameters, extracts file paths from the shell command text.
///
/// Returns `Some(reason)` if any path is blocked, `None` if all are allowed.
fn check_tool_paths(input: &Value) -> Option<String> {
    use clankers_util::path_policy::check_path;

    // Direct path parameters used by read, write, edit, ls, find, grep, etc.
    for key in ["path", "file", "directory", "cwd"] {
        if let Some(reason) = input.get(key).and_then(|v| v.as_str()).and_then(check_path) {
            return Some(reason);
        }
    }

    // Shell commands: scan for paths that look sensitive.
    // We check each whitespace-delimited token that starts with / or ~.
    if let Some(cmd) = input.get("command").and_then(|v| v.as_str()) {
        for token in cmd.split_whitespace() {
            if (token.starts_with('/') || token.starts_with("~/"))
                && let Some(reason) = check_path(token)
            {
                return Some(reason);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use chrono::Utc;
    use clanker_message::StopReason;
    use clanker_message::transcript::AssistantMessage;
    use clanker_message::transcript::BashExecutionMessage;
    use clanker_message::transcript::BranchSummaryMessage;
    use clanker_message::transcript::CompactionSummaryMessage;
    use clanker_message::transcript::CustomMessage;
    use clanker_message::transcript::MessageId;
    use clanker_message::transcript::ToolResultMessage;
    use clanker_message::transcript::UserMessage;
    use clankers_engine::EngineCorrelationId;
    use serde_json::json;

    use super::*;
    use crate::tool::ToolResult as AgentToolResult;

    const TEST_MAX_TOKENS: usize = 128;
    const TEST_THINKING_BUDGET: usize = 256;
    const TEST_TOKENS_SAVED: usize = 512;
    const TEST_TOOL_NAME: &str = "test_tool";
    const TEST_TOOL_DESCRIPTION: &str = "test description";
    const TEST_CALL_ID: &str = "call-1";

    struct FakeTool {
        definition: crate::tool::ToolDefinition,
    }

    struct PanicTool {
        definition: crate::tool::ToolDefinition,
    }

    struct NeutralStorageSearchProgressTool {
        definition: crate::tool::ToolDefinition,
    }

    struct FakeNeutralStorage {
        writes: std::sync::Mutex<Vec<clankers_tool_host::ToolStorageWriteRequest>>,
    }

    struct FakeNeutralSearch {
        queries: std::sync::Mutex<Vec<String>>,
    }

    struct PanicLegacyRunner;

    impl FakeTool {
        fn new() -> Self {
            Self {
                definition: crate::tool::ToolDefinition {
                    name: TEST_TOOL_NAME.to_string(),
                    description: TEST_TOOL_DESCRIPTION.to_string(),
                    input_schema: json!({"type": "object"}),
                },
            }
        }
    }

    impl PanicTool {
        fn new() -> Self {
            Self {
                definition: crate::tool::ToolDefinition {
                    name: "panic_tool".to_string(),
                    description: "panics if executed".to_string(),
                    input_schema: json!({"type": "object"}),
                },
            }
        }
    }

    impl NeutralStorageSearchProgressTool {
        fn new() -> Self {
            Self {
                definition: crate::tool::ToolDefinition {
                    name: "neutral_storage_search".to_string(),
                    description: "neutral storage/search/progress test tool".to_string(),
                    input_schema: json!({"type": "object"}),
                },
            }
        }
    }

    struct SourceTool {
        definition: crate::tool::ToolDefinition,
        source: String,
    }

    impl SourceTool {
        fn new(name: &str, source: &str) -> Self {
            Self {
                definition: crate::tool::ToolDefinition {
                    name: name.to_string(),
                    description: format!("{source} test tool"),
                    input_schema: json!({"type": "object"}),
                },
                source: source.to_string(),
            }
        }
    }

    #[async_trait]
    impl Tool for SourceTool {
        fn definition(&self) -> &crate::tool::ToolDefinition {
            &self.definition
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> AgentToolResult {
            AgentToolResult::text(format!("{}:{}", self.source, self.definition.name))
        }

        fn source(&self) -> &str {
            &self.source
        }
    }

    #[async_trait]
    impl Tool for FakeTool {
        fn definition(&self) -> &crate::tool::ToolDefinition {
            &self.definition
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> AgentToolResult {
            AgentToolResult::text("ok")
        }
    }

    #[async_trait]
    impl Tool for PanicTool {
        fn definition(&self) -> &crate::tool::ToolDefinition {
            &self.definition
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> AgentToolResult {
            panic!("blocked Steel substrate calls must not execute direct tool path")
        }
    }

    #[async_trait]
    impl Tool for NeutralStorageSearchProgressTool {
        fn definition(&self) -> &crate::tool::ToolDefinition {
            &self.definition
        }

        async fn execute(&self, _ctx: &ToolContext, _params: Value) -> AgentToolResult {
            panic!("neutral tool path must not use legacy ToolContext")
        }

        fn uses_neutral_tool_context(&self) -> bool {
            true
        }

        async fn execute_with_neutral_context(
            &self,
            context: clankers_tool_host::ToolInvocationContext,
            params: Value,
        ) -> AgentToolResult {
            for service in [
                clankers_tool_host::ToolHostServiceKind::Storage,
                clankers_tool_host::ToolHostServiceKind::Search,
            ] {
                if let Err(outcome) = context.require_service(&self.definition.name, service) {
                    return AgentToolResult::error(format!("missing neutral service: {outcome:?}"));
                }
            }
            let value = params.get("value").and_then(Value::as_str).unwrap_or("missing");
            context
                .emit_progress(clankers_tool_host::ToolProgressKind::Progress, format!("neutral value: {value}"))
                .ok();
            let key = clankers_tool_host::ToolStorageKey::new("fixture", "value");
            let storage = context.storage.as_ref().expect("storage service present");
            storage
                .write(clankers_tool_host::ToolStorageWriteRequest {
                    key: key.clone(),
                    value: clankers_tool_host::ToolStorageValue::new(value.as_bytes().to_vec()),
                })
                .expect("storage write should pass");
            let stored = storage
                .read(clankers_tool_host::ToolStorageReadRequest { key })
                .expect("storage read should pass")
                .value
                .expect("stored value should exist");
            let search = context.search.as_ref().expect("search service present");
            let hits = search
                .search(clankers_tool_host::ToolSearchRequest::new(value, 1))
                .expect("search should pass")
                .hits;
            let stored_text = String::from_utf8(stored.bytes).expect("storage bytes should be utf8");
            AgentToolResult::text(format!("stored={stored_text}; search={}", hits[0].snippet))
        }
    }

    impl clankers_tool_host::ToolStorageService for FakeNeutralStorage {
        fn read(
            &self,
            _request: clankers_tool_host::ToolStorageReadRequest,
        ) -> std::result::Result<clankers_tool_host::ToolStorageReadResult, clankers_tool_host::ToolHostError> {
            let writes = self.writes.lock().expect("storage lock");
            let value = writes.last().map(|write| write.value.clone());
            Ok(clankers_tool_host::ToolStorageReadResult { value })
        }

        fn write(
            &self,
            request: clankers_tool_host::ToolStorageWriteRequest,
        ) -> std::result::Result<clankers_tool_host::ToolStorageWriteResult, clankers_tool_host::ToolHostError>
        {
            self.writes.lock().expect("storage lock").push(request);
            Ok(clankers_tool_host::ToolStorageWriteResult {
                stored: true,
                metadata: std::collections::BTreeMap::new(),
            })
        }
    }

    impl clankers_tool_host::ToolSearchService for FakeNeutralSearch {
        fn search(
            &self,
            request: clankers_tool_host::ToolSearchRequest,
        ) -> std::result::Result<clankers_tool_host::ToolSearchResult, clankers_tool_host::ToolHostError> {
            self.queries.lock().expect("search lock").push(request.query.clone());
            Ok(clankers_tool_host::ToolSearchResult {
                hits: vec![clankers_tool_host::ToolSearchHit::new("fixture", request.query, 1)],
            })
        }
    }

    #[async_trait]
    impl LegacyToolRunner for PanicLegacyRunner {
        async fn execute_legacy_tool(&self, _tool: Arc<dyn Tool>, _call_id: String, _input: Value) -> ToolExecResult {
            panic!("neutral tool path must not invoke legacy runner")
        }
    }

    fn timestamp() -> chrono::DateTime<chrono::Utc> {
        Utc::now()
    }

    fn text_content(text: &str) -> Vec<Content> {
        vec![Content::Text { text: text.to_string() }]
    }

    fn user_message() -> AgentMessage {
        AgentMessage::User(UserMessage {
            id: MessageId::new("user-1"),
            content: text_content("hello"),
            timestamp: timestamp(),
        })
    }

    fn assistant_message() -> AgentMessage {
        AgentMessage::Assistant(AssistantMessage {
            id: MessageId::new("assistant-1"),
            content: text_content("hi"),
            model: "test-model".to_string(),
            usage: Usage::default(),
            stop_reason: StopReason::Stop,
            timestamp: timestamp(),
        })
    }

    fn tool_result_message() -> AgentMessage {
        AgentMessage::ToolResult(ToolResultMessage {
            id: MessageId::new("tool-1"),
            call_id: "call-1".to_string(),
            tool_name: "read".to_string(),
            content: text_content("tool output"),
            is_error: true,
            details: None,
            timestamp: timestamp(),
        })
    }

    fn engine_model_request(messages: Vec<EngineMessage>) -> EngineModelRequest {
        EngineModelRequest {
            request_id: EngineCorrelationId("request-1".to_string()),
            model: "test-model".to_string(),
            messages,
            system_prompt: "system".to_string(),
            max_tokens: Some(TEST_MAX_TOKENS),
            temperature: None,
            thinking: Some(clanker_message::ThinkingConfig {
                enabled: true,
                budget_tokens: Some(TEST_THINKING_BUDGET),
            }),
            tools: Vec::new(),
            no_cache: true,
            cache_ttl: None,
            session_id: "session-1".to_string(),
        }
    }

    #[tokio::test]
    async fn built_in_wasm_and_stdio_tools_share_executor_seam() {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        let sources = [
            ("builtin_tool", "built-in"),
            ("wasm_tool", "wasm-plugin"),
            ("stdio_tool", "stdio-plugin"),
        ];
        for (name, source) in sources {
            tools.insert(name.to_string(), Arc::new(SourceTool::new(name, source)));
        }
        let tool_calls: Vec<(String, String, Value)> = ["builtin_tool", "wasm_tool", "stdio_tool"]
            .into_iter()
            .map(|name| (format!("call-{name}"), name.to_string(), json!({})))
            .collect();
        let (event_tx, _rx) = broadcast::channel(16);
        let cancel = CancellationToken::new();

        let results =
            execute_tools_parallel(&tools, &tool_calls, &event_tx, cancel, None, "session", None, None, None).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|message| !message.is_error));
        let rendered: Vec<String> = results
            .iter()
            .map(|message| first_text_block(&message.content).expect("tool output should be text"))
            .collect();
        assert_eq!(rendered, vec![
            "built-in:builtin_tool",
            "wasm-plugin:wasm_tool",
            "stdio-plugin:stdio_tool"
        ]);
    }

    #[tokio::test]
    async fn controller_tool_executor_runs_neutral_storage_search_progress_path() {
        let storage = Arc::new(FakeNeutralStorage {
            writes: std::sync::Mutex::new(Vec::new()),
        });
        let search = Arc::new(FakeNeutralSearch {
            queries: std::sync::Mutex::new(Vec::new()),
        });
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("neutral_storage_search".to_string(), Arc::new(NeutralStorageSearchProgressTool::new()));
        let tool_calls =
            vec![("call-neutral".to_string(), "neutral_storage_search".to_string(), json!({"value": "needle"}))];
        let (event_tx, mut rx) = broadcast::channel(16);
        let services = ControllerToolServices {
            events: Arc::new(BroadcastAgentToolEventSink {
                event_tx: event_tx.clone(),
            }),
            progress: Arc::new(AgentEventProgressSink {
                event_tx: event_tx.clone(),
            }),
            cancellation: Arc::new(TokenToolCancellationService {
                cancel: CancellationToken::new(),
            }),
            storage: Some(storage.clone()),
            search: Some(search.clone()),
            hooks: None,
            capability: None,
            legacy_runner: Arc::new(PanicLegacyRunner),
            steel_tool_substrate: None,
        };

        let results = execute_tools_parallel_with_substrate(&tools, &tool_calls, services).await;

        assert_eq!(results.len(), 1);
        assert!(!results[0].is_error);
        assert!(
            matches!(&results[0].content[0], Content::Text { text } if text.contains("stored=needle; search=needle"))
        );
        assert_eq!(storage.writes.lock().expect("storage lock").len(), 1);
        assert_eq!(search.queries.lock().expect("search lock").as_slice(), &["needle".to_string()]);
        let mut saw_progress = false;
        while let Ok(event) = rx.try_recv() {
            if let AgentEvent::ToolExecutionUpdate { call_id, partial } = event {
                saw_progress = call_id == "call-neutral"
                    && matches!(&partial.content[0], crate::tool::ToolResultContent::Text { text } if text.contains("neutral value: needle"));
            }
        }
        assert!(saw_progress, "expected neutral progress event");
    }

    #[test]
    fn concrete_controller_services_expose_db_memory_search_service() {
        let db = clankers_db::Db::in_memory().expect("in-memory db");
        db.memory()
            .save(&clankers_db::memory::MemoryEntry::new(
                "neutral service search memory",
                clankers_db::memory::MemoryScope::Global,
            ))
            .expect("save memory fixture");
        let (event_tx, _rx) = broadcast::channel(16);
        let services = ControllerToolServices::from_concrete(
            event_tx,
            CancellationToken::new(),
            None,
            "session".to_string(),
            Some(db),
            None,
            None,
            None,
        );
        let context = services.invocation_context("call-search");

        assert!(context.services.is_available(clankers_tool_host::ToolHostServiceKind::Search));
        let hits = context
            .search
            .expect("search service")
            .search(clankers_tool_host::ToolSearchRequest::new("neutral", 1).with_metadata("scope", "global"))
            .expect("search should pass")
            .hits;
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].snippet, "neutral service search memory");
        assert_eq!(hits[0].metadata.get("scope").map(String::as_str), Some("global"));
    }

    #[test]
    fn provider_stream_normalizer_feeds_host_accumulator() {
        let mut normalizer = ProviderStreamNormalizer::new();
        let provider_events = vec![
            StreamEvent::MessageStart {
                message: MessageMetadata {
                    id: "msg-1".to_string(),
                    model: "test-model".to_string(),
                    role: "assistant".to_string(),
                },
            },
            StreamEvent::ContentBlockStart {
                index: 0,
                content_block: Content::Text { text: String::new() },
            },
            StreamEvent::ContentBlockDelta {
                index: 0,
                delta: ContentDelta::TextDelta {
                    text: "hello".to_string(),
                },
            },
            StreamEvent::ContentBlockStop { index: 0 },
            StreamEvent::MessageDelta {
                stop_reason: Some("stop".to_string()),
                usage: Usage {
                    input_tokens: 1,
                    output_tokens: 2,
                    cache_creation_input_tokens: 0,
                    cache_read_input_tokens: 0,
                },
            },
            StreamEvent::MessageStop,
        ];
        let mut accumulator = clankers_engine_host::stream::StreamAccumulator::new();

        for provider_event in provider_events {
            for host_event in normalizer.push(provider_event) {
                accumulator.push(host_event).expect("host event should fold");
            }
        }
        let folded = accumulator.finish().expect("stream should finish");

        assert_eq!(folded.model.as_deref(), Some("test-model"));
        assert_eq!(folded.stop_reason, Some(StopReason::Stop));
        assert_eq!(folded.usage.expect("usage should exist").output_tokens, 2);
        assert!(matches!(&folded.content[0], Content::Text { text } if text == "hello"));
    }

    #[tokio::test]
    async fn steel_tool_substrate_blocks_before_direct_tool_execution() {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert("panic_tool".to_string(), Arc::new(PanicTool::new()));
        let tool_calls = vec![("call-blocked".to_string(), "panic_tool".to_string(), json!({}))];
        let (event_tx, _rx) = broadcast::channel(16);
        let mut profile = clankers_runtime::SteelToolSubstrateProfile::default_enabled();
        profile.fallback_mode = clankers_runtime::SteelToolSubstrateFallbackMode::Block;
        profile.allowed_executor_kinds.remove(&clankers_runtime::SteelToolExecutorKind::RustBuiltin);
        let config = AgentToolSteelSubstrateConfig {
            profile,
            steel_source: "(host \"steel.host.tool.call\")".to_string(),
            session_capabilities: vec!["steel-tool-substrate".to_string(), "tool-dispatch".to_string()],
            granted_ucan_abilities: vec!["clankers/steel/tool.call".to_string()],
            disabled_actions: Vec::new(),
        };

        let services = ControllerToolServices::from_concrete(
            event_tx.clone(),
            CancellationToken::new(),
            None,
            "session".to_string(),
            None,
            None,
            None,
            Some(config),
        );
        let results = execute_tools_parallel_with_substrate(&tools, &tool_calls, services).await;

        assert_eq!(results.len(), 1);
        assert!(results[0].is_error);
        assert!(
            first_text_block(&results[0].content)
                .expect("blocked result text")
                .contains("Steel tool substrate blocked")
        );
    }

    #[test]
    fn agent_tool_catalog_lists_metadata_and_contains_names() {
        let mut tools: HashMap<String, Arc<dyn Tool>> = HashMap::new();
        tools.insert(TEST_TOOL_NAME.to_string(), Arc::new(FakeTool::new()));
        let catalog = AgentToolCatalog {
            controller_tools: &tools,
        };

        let descriptors = catalog.describe_tools();

        assert_eq!(descriptors.len(), 1);
        assert_eq!(descriptors[0].name, TEST_TOOL_NAME);
        assert_eq!(descriptors[0].description, TEST_TOOL_DESCRIPTION);
        assert!(catalog.contains_tool(TEST_TOOL_NAME));
        assert!(!catalog.contains_tool("missing"));
        assert_eq!(tool_definitions_from_tool_catalog(&tools).len(), 1);
    }

    #[test]
    fn tool_host_outcome_round_trips_success_and_error_messages() {
        let success_message = ToolResultMessage {
            id: MessageId::new("tool-1"),
            call_id: TEST_CALL_ID.to_string(),
            tool_name: TEST_TOOL_NAME.to_string(),
            content: text_content("ok"),
            is_error: false,
            details: Some(json!({"detail": true})),
            timestamp: timestamp(),
        };
        let success_outcome = tool_result_message_to_host_outcome(&success_message);
        let success_roundtrip = tool_result_message_from_host_outcome(
            TEST_CALL_ID.to_string(),
            TEST_TOOL_NAME.to_string(),
            success_outcome,
        );
        assert!(!success_roundtrip.is_error);
        assert_eq!(success_roundtrip.content.len(), 1);
        assert_eq!(success_roundtrip.details, Some(json!({"detail": true})));

        let error_message = ToolResultMessage {
            id: MessageId::new("tool-2"),
            call_id: TEST_CALL_ID.to_string(),
            tool_name: TEST_TOOL_NAME.to_string(),
            content: text_content("bad"),
            is_error: true,
            details: None,
            timestamp: timestamp(),
        };
        let error_outcome = tool_result_message_to_host_outcome(&error_message);
        let error_roundtrip =
            tool_result_message_from_host_outcome(TEST_CALL_ID.to_string(), TEST_TOOL_NAME.to_string(), error_outcome);
        assert!(error_roundtrip.is_error);
        assert_eq!(error_roundtrip.content.len(), 1);
        assert!(error_roundtrip.details.is_none());
    }

    #[test]
    fn engine_messages_from_agent_messages_preserves_conversation_variants() {
        let converted =
            engine_messages_from_agent_messages(&[user_message(), assistant_message(), tool_result_message()]);

        assert_eq!(converted.len(), 3);
        assert_eq!(converted[0].role, EngineMessageRole::User);
        assert_eq!(converted[1].role, EngineMessageRole::Assistant);
        assert_eq!(converted[2].role, EngineMessageRole::Tool);
        assert!(matches!(
            &converted[2].content[0],
            Content::ToolResult {
                tool_use_id,
                is_error: Some(true),
                ..
            } if tool_use_id == "call-1"
        ));
    }

    #[test]
    fn engine_messages_from_agent_messages_excludes_shell_only_variants() {
        let converted = engine_messages_from_agent_messages(&[
            AgentMessage::BashExecution(BashExecutionMessage {
                id: MessageId::new("bash-1"),
                command: "echo hi".to_string(),
                stdout: "hi".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                timestamp: timestamp(),
            }),
            AgentMessage::Custom(CustomMessage {
                id: MessageId::new("custom-1"),
                kind: "meta".to_string(),
                data: json!({"ignored": true}),
                timestamp: timestamp(),
            }),
            AgentMessage::BranchSummary(BranchSummaryMessage {
                id: MessageId::new("branch-1"),
                from_id: MessageId::new("user-1"),
                summary: "branch".to_string(),
                timestamp: timestamp(),
            }),
            AgentMessage::CompactionSummary(CompactionSummaryMessage {
                id: MessageId::new("compact-1"),
                compacted_ids: vec![MessageId::new("user-1")],
                summary: "compact".to_string(),
                tokens_saved: TEST_TOKENS_SAVED,
                timestamp: timestamp(),
            }),
        ]);

        assert!(converted.is_empty());
    }

    #[test]
    fn completion_request_from_engine_request_converts_native_provider_messages() {
        let request = engine_model_request(vec![
            EngineMessage {
                role: EngineMessageRole::User,
                content: text_content("hello"),
            },
            EngineMessage {
                role: EngineMessageRole::Assistant,
                content: text_content("hi"),
            },
            EngineMessage {
                role: EngineMessageRole::Tool,
                content: vec![Content::ToolResult {
                    tool_use_id: "call-1".to_string(),
                    content: text_content("tool output"),
                    is_error: Some(false),
                }],
            },
        ]);

        let completion = completion_request_from_engine_request(&request).expect("request should convert");

        assert_eq!(completion.messages.len(), 3);
        assert!(matches!(completion.messages[0], AgentMessage::User(_)));
        assert!(matches!(completion.messages[1], AgentMessage::Assistant(_)));
        assert!(matches!(completion.messages[2], AgentMessage::ToolResult(_)));
        assert_eq!(completion.extra_params.get("_session_id"), Some(&Value::String("session-1".to_string())));
    }

    #[test]
    fn completion_request_from_engine_request_rejects_malformed_tool_message() {
        let request = engine_model_request(vec![EngineMessage {
            role: EngineMessageRole::Tool,
            content: text_content("not a tool result"),
        }]);

        let error = completion_request_from_engine_request(&request).expect_err("malformed tool message should fail");

        assert!(error.to_string().contains("tool-role message without a tool_result"));
    }
}
