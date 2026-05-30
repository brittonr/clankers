//! Host-facing session identifiers, options, and handles.

use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

use chrono::Utc;
use clanker_message::Content;
use clanker_message::ToolDefinition;
use clanker_message::Usage;
use clankers_engine::EngineCorrelationId;
use clankers_engine::EngineEvent;
use clankers_engine::EngineInput;
use clankers_engine::EngineMessage;
use clankers_engine::EngineMessageRole;
use clankers_engine::EngineModelRequest;
use clankers_engine::EngineModelResponse;
use clankers_engine::EnginePromptSubmission;
use clankers_engine::EngineState;
use clankers_engine::EngineTerminalFailure;
use clankers_engine::EngineToolCall;
use clankers_engine::reduce;
use clankers_engine_host::CancellationSource;
use clankers_engine_host::EngineEventSink;
use clankers_engine_host::EngineRunSeed;
use clankers_engine_host::HostAdapterError;
use clankers_engine_host::HostAdapters;
use clankers_engine_host::ModelHost;
use clankers_engine_host::ModelHostOutcome;
use clankers_engine_host::RetrySleeper;
use clankers_engine_host::UsageObservation;
use clankers_engine_host::UsageObserver;
use clankers_engine_host::run_engine_turn;
use clankers_tool_host::ToolExecutor;
use clankers_tool_host::ToolHostOutcome;
use serde::Deserialize;
use serde::Serialize;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::AssembledPrompt;
use crate::EventMetadata;
use crate::ModelAdapter;
use crate::ModelRequest;
use crate::ModelRequestMetadata;
use crate::PromptAssembler;
use crate::PromptId;
use crate::PromptInput;
use crate::PromptReceipt;
use crate::PromptReplayEntry;
use crate::PromptSourceRequest;
use crate::RuntimeCancellationAdapter;
use crate::RuntimeError;
use crate::RuntimeEventObserver;
use crate::RuntimeRetryAdapter;
use crate::RuntimeRetryRequest;
use crate::RuntimeToolAdapter;
use crate::RuntimeToolRequest;
use crate::RuntimeToolResponse;
use crate::RuntimeToolStatus;
use crate::RuntimeUsageAdapter;
use crate::RuntimeUsageObservation;
use crate::RuntimeUsageObservationKind;
use crate::SessionEvent;
use crate::SessionLedgerEntry;
use crate::SessionRecord;
use crate::StopReason;
use crate::ToolCatalog;
use crate::ledger_entries_from_engine_messages;
use crate::ledger_messages_from_engine_messages;
use crate::runtime::RuntimeInner;

/// Stable identifier for a host-facing runtime session.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SessionId(String);

impl SessionId {
    /// Generate a fresh session id for an embedded host.
    #[must_use]
    pub fn new() -> Self {
        Self(format!("session_{}", Uuid::new_v4()))
    }

    /// Build a session id from host-owned storage.
    #[must_use]
    pub fn from_host(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Return the stable id string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Options used when creating an embedded session.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionOptions {
    pub session_id: Option<SessionId>,
    pub model: Option<String>,
}

/// Host-facing session handle.
#[derive(Clone)]
pub struct SessionHandle {
    runtime: Arc<RuntimeInner>,
    /// Lock order: acquire `state` before `events` if both are ever needed.
    state: Arc<Mutex<SessionState>>,
    /// Lock order: acquire after `state`; current methods take only one lock at a time.
    events: Arc<Mutex<Option<mpsc::Receiver<SessionEvent>>>>,
    tx: mpsc::Sender<SessionEvent>,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: SessionId,
    model: Option<String>,
    disabled_tools: BTreeSet<String>,
    is_shutdown: bool,
    resume_required: bool,
    persist_session: bool,
}

struct RuntimeModelHost {
    model: Arc<dyn ModelAdapter>,
    request: ModelRequest,
    event_log: Arc<StdMutex<Vec<SessionEvent>>>,
}

impl RuntimeModelHost {
    fn new(model: Arc<dyn ModelAdapter>, request: ModelRequest, event_log: Arc<StdMutex<Vec<SessionEvent>>>) -> Self {
        Self {
            model,
            request,
            event_log,
        }
    }
}

impl ModelHost for RuntimeModelHost {
    async fn execute_model(&mut self, request: EngineModelRequest) -> ModelHostOutcome {
        let mut model_request = self.request.clone();
        model_request.model = Some(request.model.clone());
        model_request.history = ledger_messages_from_engine_messages(&request.messages);
        model_request.metadata = model_request_metadata(&request);
        match self.model.complete(model_request) {
            Ok(response) => {
                if let Some(failure) = response.failure {
                    return ModelHostOutcome::Failed {
                        failure: EngineTerminalFailure {
                            message: failure.message,
                            status: failure.status,
                            retryable: failure.retryable,
                        },
                    };
                }
                let (mut content, event_usage, events, failure) = model_response_to_engine_parts(response.events);
                if let Some(message) = failure {
                    return ModelHostOutcome::Failed {
                        failure: EngineTerminalFailure {
                            message,
                            status: None,
                            retryable: false,
                        },
                    };
                }
                if !response.engine_content.is_empty() {
                    content = response.engine_content;
                }
                self.event_log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).extend(events);
                ModelHostOutcome::Completed {
                    response: EngineModelResponse {
                        output: content,
                        stop_reason: response.stop_reason.unwrap_or(clanker_message::StopReason::Stop),
                    },
                    usage: response.usage.or(event_usage),
                }
            }
            Err(error) => ModelHostOutcome::Failed {
                failure: EngineTerminalFailure {
                    message: error.safe_message(),
                    status: None,
                    retryable: false,
                },
            },
        }
    }
}

struct RuntimeToolHost {
    session_id: SessionId,
    prompt_id: PromptId,
    catalog: ToolCatalog,
    adapter: Arc<dyn RuntimeToolAdapter>,
    event_log: Arc<StdMutex<Vec<SessionEvent>>>,
}

impl RuntimeToolHost {
    fn new(
        session_id: SessionId,
        prompt_id: PromptId,
        catalog: ToolCatalog,
        adapter: Arc<dyn RuntimeToolAdapter>,
        event_log: Arc<StdMutex<Vec<SessionEvent>>>,
    ) -> Self {
        Self {
            session_id,
            prompt_id,
            catalog,
            adapter,
            event_log,
        }
    }
}

impl ToolExecutor for RuntimeToolHost {
    async fn execute_tool(&mut self, call: EngineToolCall) -> ToolHostOutcome {
        if !self.catalog.contains_tool(&call.tool_name) {
            return ToolHostOutcome::MissingTool { name: call.tool_name };
        }
        push_runtime_tool_event(&self.event_log, SessionEvent::ToolStarted {
            prompt_id: self.prompt_id.clone(),
            call_id: call.call_id.0.clone(),
            tool_name: call.tool_name.clone(),
            metadata: EventMetadata::new(self.session_id.clone()).with("source", "runtime_tool_host"),
        });
        let request = RuntimeToolRequest {
            session_id: self.session_id.clone(),
            prompt_id: self.prompt_id.clone(),
            call_id: call.call_id.0.clone(),
            tool_name: call.tool_name.clone(),
            input: call.input,
        };
        match self.adapter.execute_tool(request) {
            Ok(response) => {
                push_runtime_tool_event(&self.event_log, SessionEvent::ToolFinished {
                    prompt_id: self.prompt_id.clone(),
                    call_id: call.call_id.0.clone(),
                    status: runtime_tool_status_to_session_status(response.status),
                    metadata: EventMetadata::new(self.session_id.clone())
                        .with("runtime_tool_status", format!("{:?}", response.status)),
                });
                runtime_tool_response_to_host_outcome(call.tool_name, response)
            }
            Err(error) => {
                let message = error.safe_message();
                push_runtime_tool_event(&self.event_log, SessionEvent::ToolFinished {
                    prompt_id: self.prompt_id.clone(),
                    call_id: call.call_id.0,
                    status: crate::ToolStatus::Failed,
                    metadata: EventMetadata::new(self.session_id.clone())
                        .with("error_class", format!("{:?}", error.class())),
                });
                ToolHostOutcome::ToolError {
                    content: vec![Content::Text { text: message.clone() }],
                    details: serde_json::json!({"error_class": format!("{:?}", error.class())}),
                    message,
                }
            }
        }
    }
}

fn push_runtime_tool_event(event_log: &Arc<StdMutex<Vec<SessionEvent>>>, event: SessionEvent) {
    event_log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(event);
}

fn runtime_tool_status_to_session_status(status: RuntimeToolStatus) -> crate::ToolStatus {
    match status {
        RuntimeToolStatus::Succeeded => crate::ToolStatus::Succeeded,
        RuntimeToolStatus::Denied => crate::ToolStatus::Denied,
        RuntimeToolStatus::Failed | RuntimeToolStatus::Missing | RuntimeToolStatus::Cancelled => {
            crate::ToolStatus::Failed
        }
    }
}

fn runtime_tool_response_to_host_outcome(tool_name: String, response: RuntimeToolResponse) -> ToolHostOutcome {
    match response.status {
        RuntimeToolStatus::Succeeded => ToolHostOutcome::Succeeded {
            content: response.content,
            details: response.details,
        },
        RuntimeToolStatus::Denied => ToolHostOutcome::CapabilityDenied {
            name: tool_name,
            reason: response.message.unwrap_or_else(|| "denied by runtime tool adapter".to_string()),
        },
        RuntimeToolStatus::Missing => ToolHostOutcome::MissingTool { name: tool_name },
        RuntimeToolStatus::Cancelled => ToolHostOutcome::Cancelled { name: tool_name },
        RuntimeToolStatus::Failed => {
            let message = response.message.unwrap_or_else(|| "runtime tool adapter failed".to_string());
            let content = if response.content.is_empty() {
                vec![Content::Text { text: message.clone() }]
            } else {
                response.content
            };
            ToolHostOutcome::ToolError {
                content,
                details: response.details,
                message,
            }
        }
    }
}

struct RuntimeRetrySleeper {
    adapter: Arc<dyn RuntimeRetryAdapter>,
}

impl RetrySleeper for RuntimeRetrySleeper {
    async fn sleep_for_retry(
        &mut self,
        request_id: EngineCorrelationId,
        delay: std::time::Duration,
    ) -> Result<(), HostAdapterError> {
        self.adapter
            .sleep_for_retry(RuntimeRetryRequest::new(request_id.0, delay))
            .map_err(|error| HostAdapterError::failed(error.safe_message()))
    }
}

struct RuntimeEngineEventSink {
    observer: Arc<dyn RuntimeEventObserver>,
    events: Vec<EngineEvent>,
}

impl EngineEventSink for RuntimeEngineEventSink {
    fn emit_engine_event(&mut self, event: &EngineEvent) -> Result<(), HostAdapterError> {
        self.events.push(event.clone());
        self.observer
            .observe_engine_event(event)
            .map_err(|error| HostAdapterError::failed(error.safe_message()))
    }
}

struct RuntimeCancellationSource {
    adapter: Arc<dyn RuntimeCancellationAdapter>,
}

impl CancellationSource for RuntimeCancellationSource {
    fn is_cancelled(&mut self) -> bool {
        self.adapter.is_cancelled()
    }

    fn cancellation_reason(&mut self) -> String {
        self.adapter.cancellation_reason()
    }
}

struct RuntimeUsageObserver {
    adapter: Arc<dyn RuntimeUsageAdapter>,
    session_id: SessionId,
    prompt_id: PromptId,
    event_log: Arc<StdMutex<Vec<SessionEvent>>>,
    observations: Vec<UsageObservation>,
}

impl UsageObserver for RuntimeUsageObserver {
    fn observe_usage(&mut self, observation: &UsageObservation) -> Result<(), HostAdapterError> {
        self.observations.push(observation.clone());
        push_runtime_tool_event(&self.event_log, SessionEvent::CostUpdated {
            prompt_id: self.prompt_id.clone(),
            input_tokens: observation.usage.input_tokens as u64,
            output_tokens: observation.usage.output_tokens as u64,
            metadata: EventMetadata::new(self.session_id.clone()).with("source", "engine_host_usage"),
        });
        self.adapter
            .observe_usage(runtime_usage_observation(observation))
            .map_err(|error| HostAdapterError::failed(error.safe_message()))
    }
}

fn runtime_usage_observation(observation: &UsageObservation) -> RuntimeUsageObservation {
    RuntimeUsageObservation {
        kind: match observation.kind {
            clankers_engine_host::UsageObservationKind::StreamDelta => RuntimeUsageObservationKind::StreamDelta,
            clankers_engine_host::UsageObservationKind::FinalSummary => RuntimeUsageObservationKind::FinalSummary,
        },
        usage: observation.usage.clone(),
    }
}

fn runtime_usage_tokens_to_usize(tokens: u64) -> Result<usize, String> {
    usize::try_from(tokens).map_err(|_| "runtime usage token count exceeded host size".to_string())
}

fn model_response_to_engine_parts(
    events: Vec<SessionEvent>,
) -> (Vec<Content>, Option<Usage>, Vec<SessionEvent>, Option<String>) {
    let mut assistant_text = String::new();
    let mut thinking_text = String::new();
    let mut usage = None;
    let mut replay_events = Vec::new();
    let mut failure = None;

    for event in events {
        match &event {
            SessionEvent::AssistantDelta { text, .. } => assistant_text.push_str(text),
            SessionEvent::ThinkingDelta { text, .. } => thinking_text.push_str(text),
            SessionEvent::CostUpdated {
                input_tokens,
                output_tokens,
                ..
            } => match (runtime_usage_tokens_to_usize(*input_tokens), runtime_usage_tokens_to_usize(*output_tokens)) {
                (Ok(input_tokens), Ok(output_tokens)) => {
                    usage = Some(Usage {
                        input_tokens,
                        output_tokens,
                        cache_creation_input_tokens: 0,
                        cache_read_input_tokens: 0,
                    });
                }
                (Err(message), _) | (_, Err(message)) => failure = Some(message),
            },
            SessionEvent::Error { message, .. } => failure = Some(message.clone()),
            SessionEvent::PromptAccepted { .. } | SessionEvent::Completed { .. } | SessionEvent::Shutdown { .. } => {}
            SessionEvent::ToolStarted { .. }
            | SessionEvent::ToolFinished { .. }
            | SessionEvent::ConfirmationRequested { .. } => {}
        }
        if should_replay_model_event(&event) {
            replay_events.push(event);
        }
    }

    let mut content = Vec::new();
    if !thinking_text.is_empty() {
        content.push(Content::Thinking {
            thinking: thinking_text,
            signature: String::new(),
        });
    }
    if !assistant_text.is_empty() {
        content.push(Content::Text { text: assistant_text });
    }
    (content, usage, replay_events, failure)
}

fn should_replay_model_event(event: &SessionEvent) -> bool {
    matches!(
        event,
        SessionEvent::ThinkingDelta { .. }
            | SessionEvent::AssistantDelta { .. }
            | SessionEvent::ToolStarted { .. }
            | SessionEvent::ToolFinished { .. }
            | SessionEvent::ConfirmationRequested { .. }
    )
}

fn system_prompt_from_assembled(assembled: &AssembledPrompt) -> String {
    assembled
        .sections
        .iter()
        .map(|section| format!("[{}]\n{}", section.label, section.content))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn engine_tool_definitions(catalog: &ToolCatalog) -> Vec<ToolDefinition> {
    catalog
        .tools()
        .map(|tool| ToolDefinition {
            name: tool.name.clone(),
            description: tool.description.clone(),
            input_schema: serde_json::json!({"type":"object"}),
        })
        .collect()
}

fn model_request_metadata(request: &EngineModelRequest) -> ModelRequestMetadata {
    let mut tool_names = request.tools.iter().map(|tool| tool.name.clone()).collect::<Vec<_>>();
    tool_names.sort();
    ModelRequestMetadata {
        request_id: request.request_id.0.clone(),
        message_count: request.messages.len(),
        system_prompt: request.system_prompt.clone(),
        max_tokens: request.max_tokens,
        temperature: request.temperature,
        tool_names,
        no_cache: request.no_cache,
        cache_ttl: request.cache_ttl.clone(),
    }
}

fn save_initial_session_record(runtime: &RuntimeInner, session_id: &SessionId) -> Result<bool, RuntimeError> {
    match runtime.services.sessions.save(SessionRecord::new(session_id.clone())) {
        Ok(()) => Ok(true),
        Err(RuntimeError::SessionUnsupported(_)) => Ok(false),
        Err(error) => Err(error),
    }
}

fn replace_record_replay_messages(
    record: &mut SessionRecord,
    messages: &[EngineMessage],
    prompt_id: PromptId,
    usage: Option<Usage>,
) {
    let mut entries = record
        .ledger_entries
        .iter()
        .filter(|entry| !matches!(entry, SessionLedgerEntry::Message { .. } | SessionLedgerEntry::Summary { .. }))
        .cloned()
        .collect::<Vec<_>>();
    entries.splice(0..0, ledger_entries_from_engine_messages(messages));
    if let Some(usage) = usage {
        entries.push(SessionLedgerEntry::usage(prompt_id.clone(), usage));
    }
    entries.push(SessionLedgerEntry::receipt(
        prompt_id,
        "completed",
        EventMetadata::new(record.session_id.clone()).with("source", "runtime_session"),
    ));
    record.ledger_entries = entries;
}

impl SessionHandle {
    pub(crate) fn new(runtime: Arc<RuntimeInner>, options: SessionOptions) -> Result<Self, RuntimeError> {
        let session_id = options.session_id.unwrap_or_default();
        let persist_session = save_initial_session_record(&runtime, &session_id)?;
        Self::from_parts(runtime, session_id, options.model, false, persist_session)
    }

    pub(crate) fn resume(
        runtime: Arc<RuntimeInner>,
        session_id: SessionId,
        options: SessionOptions,
    ) -> Result<Self, RuntimeError> {
        let loaded = runtime.services.sessions.load(&session_id)?;
        if loaded.is_none() {
            return Err(RuntimeError::SessionMissing(session_id.to_string()));
        }
        Self::from_parts(runtime, session_id, options.model, true, true)
    }

    fn from_parts(
        runtime: Arc<RuntimeInner>,
        session_id: SessionId,
        model: Option<String>,
        resume_required: bool,
        persist_session: bool,
    ) -> Result<Self, RuntimeError> {
        let (tx, rx) = mpsc::channel(runtime.event_buffer);
        let state = SessionState {
            session_id,
            model,
            disabled_tools: BTreeSet::new(),
            is_shutdown: false,
            resume_required,
            persist_session,
        };
        Ok(Self {
            runtime,
            state: Arc::new(Mutex::new(state)),
            events: Arc::new(Mutex::new(Some(rx))),
            tx,
        })
    }

    /// Return the session id without exposing daemon/session protocol frames.
    pub async fn session_id(&self) -> SessionId {
        self.state.lock().await.session_id.clone()
    }

    /// Take the semantic event receiver. A session exposes one ordered event stream.
    pub async fn take_events(&self) -> Result<mpsc::Receiver<SessionEvent>, RuntimeError> {
        self.events.lock().await.take().ok_or(RuntimeError::EventStreamAlreadyTaken)
    }

    /// Submit one prompt and emit typed semantic events in causal order.
    pub async fn submit_prompt(&self, input: PromptInput) -> Result<PromptReceipt, RuntimeError> {
        let (session_id, model, disabled_tools, resume_required, persist_session) = {
            let state = self.state.lock().await;
            if state.is_shutdown {
                return Err(RuntimeError::SessionShutdown);
            }
            (
                state.session_id.clone(),
                state.model.clone(),
                state.disabled_tools.clone(),
                state.resume_required,
                state.persist_session,
            )
        };

        let sources = self.runtime.prompt_source_service.resolve_sources(PromptSourceRequest {
            user_prompt: input.text.clone(),
            policy: self.runtime.prompt_policy.clone(),
        })?;
        let assembled = PromptAssembler::assemble(&self.runtime.prompt_policy, &sources, input.text)?;
        let mut history = if resume_required {
            let record = self
                .runtime
                .services
                .sessions
                .load(&session_id)?
                .ok_or_else(|| RuntimeError::SessionMissing(session_id.to_string()))?;
            record.replay()?.messages
        } else {
            Vec::new()
        };
        let prompt_id = PromptId::new();
        let safe_metadata = EventMetadata::new(session_id.clone())
            .with("prompt_id", prompt_id.as_str())
            .with("model", model.clone().unwrap_or_else(|| "default".to_string()))
            .with("prompt_chars", assembled.user_prompt.chars().count().to_string())
            .with("disabled_tool_count", disabled_tools.len().to_string())
            .with("restored_message_count", history.len().to_string());

        self.emit(SessionEvent::PromptAccepted {
            prompt_id: prompt_id.clone(),
            metadata: safe_metadata.clone(),
        })
        .await?;

        history.push(EngineMessage {
            role: EngineMessageRole::User,
            content: vec![Content::Text {
                text: assembled.user_prompt.clone(),
            }],
        });
        let submission = EnginePromptSubmission {
            messages: history,
            model: model.clone().unwrap_or_else(|| "default".to_string()),
            system_prompt: system_prompt_from_assembled(&assembled),
            max_tokens: None,
            temperature: None,
            thinking: None,
            tools: engine_tool_definitions(&self.runtime.tool_catalog),
            no_cache: true,
            cache_ttl: None,
            session_id: session_id.to_string(),
            model_request_slot_budget: 8,
        };
        let initial_state = EngineState::new();
        let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
        let event_log = Arc::new(StdMutex::new(Vec::new()));
        let mut model_host = RuntimeModelHost::new(
            self.runtime.model.clone(),
            ModelRequest {
                session_id: session_id.clone(),
                prompt_id: prompt_id.clone(),
                model,
                prompt: assembled.clone(),
                disabled_tools,
                history: Vec::new(),
                metadata: ModelRequestMetadata::default(),
            },
            Arc::clone(&event_log),
        );
        let mut tool_host = RuntimeToolHost::new(
            session_id.clone(),
            prompt_id.clone(),
            self.runtime.tool_catalog.clone(),
            Arc::clone(&self.runtime.tool_adapter),
            Arc::clone(&event_log),
        );
        let mut retry_sleeper = RuntimeRetrySleeper {
            adapter: Arc::clone(&self.runtime.retry_adapter),
        };
        let mut event_sink = RuntimeEngineEventSink {
            observer: Arc::clone(&self.runtime.event_observer),
            events: Vec::new(),
        };
        let mut cancellation = RuntimeCancellationSource {
            adapter: Arc::clone(&self.runtime.cancellation),
        };
        let mut usage_observer = RuntimeUsageObserver {
            adapter: Arc::clone(&self.runtime.usage_adapter),
            session_id: session_id.clone(),
            prompt_id: prompt_id.clone(),
            event_log: Arc::clone(&event_log),
            observations: Vec::new(),
        };

        let report = run_engine_turn(EngineRunSeed::new(initial_state, first_outcome), HostAdapters {
            model: &mut model_host,
            tools: &mut tool_host,
            retry_sleeper: &mut retry_sleeper,
            event_sink: &mut event_sink,
            cancellation: &mut cancellation,
            usage_observer: &mut usage_observer,
        })
        .await;

        let events = std::mem::take(&mut *event_log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()));
        for event in events {
            self.emit(event.with_session_metadata(session_id.clone(), prompt_id.clone())).await?;
        }

        if let Some(failure) = report.last_outcome.terminal_failure {
            let error = RuntimeError::Model(failure.message);
            self.emit(SessionEvent::Error {
                prompt_id: Some(prompt_id.clone()),
                message: error.safe_message(),
                error_class: error.class(),
                metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
            })
            .await?;
            return Err(error);
        }

        if persist_session {
            let mut record = self
                .runtime
                .services
                .sessions
                .load(&session_id)?
                .unwrap_or_else(|| SessionRecord::new(session_id.clone()));
            record.last_prompt = Some(prompt_id.clone());
            record.prompts.push(PromptReplayEntry {
                prompt_id: prompt_id.clone(),
                user_prompt: assembled.user_prompt.clone(),
                assembled_prompt: assembled.clone(),
                completed_at: Utc::now(),
            });
            let usage = report.usage_observations.last().map(|observation| observation.usage.clone());
            replace_record_replay_messages(&mut record, &report.final_state.messages, prompt_id.clone(), usage);
            self.runtime.services.sessions.save(SessionRecord {
                session_id: session_id.clone(),
                ..record
            })?;
        }
        let stop_reason = if self.runtime.cancellation.is_cancelled() {
            StopReason::Cancelled
        } else {
            StopReason::Complete
        };
        self.emit(SessionEvent::Completed {
            prompt_id: prompt_id.clone(),
            stop_reason,
            metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
        })
        .await?;
        Ok(PromptReceipt { prompt_id })
    }

    /// Request cancellation/interrupt. The first slice emits a terminal semantic event.
    pub async fn interrupt(&self) -> Result<(), RuntimeError> {
        let session_id = self.session_id().await;
        self.emit(SessionEvent::Completed {
            prompt_id: PromptId::from_host("interrupt"),
            stop_reason: StopReason::Interrupted,
            metadata: EventMetadata::new(session_id),
        })
        .await
    }

    /// Update the preferred model for later prompts.
    pub async fn set_model(&self, model: impl Into<String>) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().await;
        if state.is_shutdown {
            return Err(RuntimeError::SessionShutdown);
        }
        state.model = Some(model.into());
        Ok(())
    }

    /// Replace the disabled tool set for later prompts.
    pub async fn set_disabled_tools(&self, tools: impl IntoIterator<Item = String>) -> Result<(), RuntimeError> {
        let mut state = self.state.lock().await;
        if state.is_shutdown {
            return Err(RuntimeError::SessionShutdown);
        }
        state.disabled_tools = tools.into_iter().collect();
        Ok(())
    }

    /// Shut down the session and emit a final typed event.
    pub async fn shutdown(&self) -> Result<(), RuntimeError> {
        let session_id = {
            let mut state = self.state.lock().await;
            state.is_shutdown = true;
            state.session_id.clone()
        };
        self.emit(SessionEvent::Shutdown {
            metadata: EventMetadata::new(session_id),
        })
        .await
    }

    async fn emit(&self, event: SessionEvent) -> Result<(), RuntimeError> {
        self.tx.send(event).await.map_err(|_| RuntimeError::EventStreamClosed)
    }
}
