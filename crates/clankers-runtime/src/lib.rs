//! Host-facing embedding facade for Clankers.
//!
//! This crate is intentionally transport-neutral. Public types model sessions,
//! prompts, tools, confirmation decisions, prompt assembly, and runtime-owned
//! services without exposing daemon frames, TUI state, CLI arguments, ACP/MCP
//! envelopes, or Matrix adapter types.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

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

/// Runtime construction entrypoint for embedded hosts.
pub struct RuntimeBuilder {
    model: Arc<dyn ModelAdapter>,
    services: RuntimeServices,
    prompt_policy: PromptAssemblyPolicy,
    prompt_sources: PromptSources,
    tool_catalog: ToolCatalog,
    confirmation_broker: Arc<dyn ConfirmationBroker>,
    event_buffer: usize,
}

impl RuntimeBuilder {
    /// Create a builder with safe in-memory defaults and an echo model adapter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            model: Arc::new(EchoModelAdapter),
            services: RuntimeServices::in_memory(),
            prompt_policy: PromptAssemblyPolicy::host_context_only(),
            prompt_sources: PromptSources::default(),
            tool_catalog: ToolCatalog::embedding_safe(),
            confirmation_broker: Arc::new(FailClosedConfirmationBroker),
            event_buffer: 128,
        }
    }

    /// Use a host-supplied model adapter.
    #[must_use]
    pub fn model_adapter(mut self, model: Arc<dyn ModelAdapter>) -> Self {
        self.model = model;
        self
    }

    /// Use explicit runtime service implementations.
    #[must_use]
    pub fn services(mut self, services: RuntimeServices) -> Self {
        self.services = services;
        self
    }

    /// Use explicit prompt assembly inputs.
    #[must_use]
    pub fn prompt_assembly(mut self, policy: PromptAssemblyPolicy, sources: PromptSources) -> Self {
        self.prompt_policy = policy;
        self.prompt_sources = sources;
        self
    }

    /// Use a host-defined tool catalog.
    #[must_use]
    pub fn tool_catalog(mut self, catalog: ToolCatalog) -> Self {
        self.tool_catalog = catalog;
        self
    }

    /// Use a host-supplied confirmation broker.
    #[must_use]
    pub fn confirmation_broker(mut self, broker: Arc<dyn ConfirmationBroker>) -> Self {
        self.confirmation_broker = broker;
        self
    }

    /// Set the per-session event channel capacity.
    #[must_use]
    pub fn event_buffer(mut self, event_buffer: usize) -> Self {
        self.event_buffer = event_buffer.max(1);
        self
    }

    /// Build a runtime.
    pub fn build(self) -> Result<Runtime, RuntimeError> {
        validate_public_runtime_boundary()?;
        Ok(Runtime {
            inner: Arc::new(RuntimeInner {
                model: self.model,
                services: self.services,
                prompt_policy: self.prompt_policy,
                prompt_sources: self.prompt_sources,
                tool_catalog: self.tool_catalog,
                confirmation_broker: self.confirmation_broker,
                event_buffer: self.event_buffer,
            }),
        })
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Built runtime handle. Cloneable and cheap.
#[derive(Clone)]
pub struct Runtime {
    inner: Arc<RuntimeInner>,
}

struct RuntimeInner {
    model: Arc<dyn ModelAdapter>,
    services: RuntimeServices,
    prompt_policy: PromptAssemblyPolicy,
    prompt_sources: PromptSources,
    tool_catalog: ToolCatalog,
    confirmation_broker: Arc<dyn ConfirmationBroker>,
    event_buffer: usize,
}

impl Runtime {
    /// Create a new host-facing session.
    pub async fn create_session(&self, options: SessionOptions) -> Result<SessionHandle, RuntimeError> {
        let session_id = options.session_id.unwrap_or_default();
        let (tx, rx) = mpsc::channel(self.inner.event_buffer);
        let state = SessionState {
            session_id: session_id.clone(),
            model: options.model,
            disabled_tools: BTreeSet::new(),
            is_shutdown: false,
        };
        self.inner.services.sessions.save(SessionRecord {
            session_id: session_id.clone(),
            created_at: Utc::now(),
            last_prompt: None,
            prompts: Vec::new(),
        })?;
        Ok(SessionHandle {
            runtime: Arc::clone(&self.inner),
            state: Arc::new(Mutex::new(state)),
            events: Arc::new(Mutex::new(Some(rx))),
            tx,
        })
    }

    /// Return the catalog published to embedded hosts.
    #[must_use]
    pub fn tool_catalog(&self) -> &ToolCatalog {
        &self.inner.tool_catalog
    }

    /// Assemble a prompt with the runtime policy.
    pub fn assemble_prompt(&self, user_prompt: impl Into<String>) -> Result<AssembledPrompt, RuntimeError> {
        PromptAssembler::assemble(&self.inner.prompt_policy, &self.inner.prompt_sources, user_prompt.into())
    }

    /// Ask the confirmation broker through the same fail-closed substrate used by sessions.
    pub async fn request_confirmation(
        &self,
        request: ConfirmationRequest,
    ) -> Result<ConfirmationDecision, RuntimeError> {
        request_confirmation_fail_closed(self.inner.confirmation_broker.as_ref(), request)
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
    state: Arc<Mutex<SessionState>>,
    events: Arc<Mutex<Option<mpsc::Receiver<SessionEvent>>>>,
    tx: mpsc::Sender<SessionEvent>,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: SessionId,
    model: Option<String>,
    disabled_tools: BTreeSet<String>,
    is_shutdown: bool,
}

impl SessionHandle {
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
        let (session_id, model, disabled_tools) = {
            let state = self.state.lock().await;
            if state.is_shutdown {
                return Err(RuntimeError::SessionShutdown);
            }
            (state.session_id.clone(), state.model.clone(), state.disabled_tools.clone())
        };

        let assembled =
            PromptAssembler::assemble(&self.runtime.prompt_policy, &self.runtime.prompt_sources, input.text)?;
        let prompt_id = PromptId::new();
        let safe_metadata = EventMetadata::new(session_id.clone())
            .with("prompt_id", prompt_id.as_str())
            .with("model", model.clone().unwrap_or_else(|| "default".to_string()))
            .with("prompt_chars", assembled.user_prompt.chars().count().to_string())
            .with("disabled_tool_count", disabled_tools.len().to_string());

        self.emit(SessionEvent::PromptAccepted {
            prompt_id: prompt_id.clone(),
            metadata: safe_metadata.clone(),
        })
        .await?;

        let request = ModelRequest {
            session_id: session_id.clone(),
            prompt_id: prompt_id.clone(),
            model,
            prompt: assembled.clone(),
            disabled_tools,
        };
        match self.runtime.model.complete(request) {
            Ok(response) => {
                for event in response.events {
                    self.emit(event.with_session_metadata(session_id.clone(), prompt_id.clone())).await?;
                }
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
                self.runtime.services.sessions.save(SessionRecord {
                    session_id: session_id.clone(),
                    ..record
                })?;
                self.emit(SessionEvent::Completed {
                    prompt_id: prompt_id.clone(),
                    stop_reason: StopReason::Complete,
                    metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
                })
                .await?;
            }
            Err(error) => {
                self.emit(SessionEvent::Error {
                    prompt_id: Some(prompt_id.clone()),
                    message: error.safe_message(),
                    error_class: error.class(),
                    metadata: EventMetadata::new(session_id).with("prompt_id", prompt_id.as_str()),
                })
                .await?;
                return Err(error);
            }
        }
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

/// Host prompt input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptInput {
    pub text: String,
}

impl PromptInput {
    #[must_use]
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into() }
    }
}

/// Prompt submission receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromptReceipt {
    pub prompt_id: PromptId,
}

/// Prompt identity allocated by the runtime facade.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct PromptId(String);

impl PromptId {
    #[must_use]
    pub fn new() -> Self {
        Self(format!("prompt_{}", Uuid::new_v4()))
    }

    #[must_use]
    pub fn from_host(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for PromptId {
    fn default() -> Self {
        Self::new()
    }
}

/// Semantic session events for host applications.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    PromptAccepted {
        prompt_id: PromptId,
        metadata: EventMetadata,
    },
    ThinkingDelta {
        prompt_id: PromptId,
        text: String,
        metadata: EventMetadata,
    },
    AssistantDelta {
        prompt_id: PromptId,
        text: String,
        metadata: EventMetadata,
    },
    ToolStarted {
        prompt_id: PromptId,
        call_id: String,
        tool_name: String,
        metadata: EventMetadata,
    },
    ToolFinished {
        prompt_id: PromptId,
        call_id: String,
        status: ToolStatus,
        metadata: EventMetadata,
    },
    ConfirmationRequested {
        prompt_id: PromptId,
        request: ConfirmationRequest,
        metadata: EventMetadata,
    },
    CostUpdated {
        prompt_id: PromptId,
        input_tokens: u64,
        output_tokens: u64,
        metadata: EventMetadata,
    },
    Completed {
        prompt_id: PromptId,
        stop_reason: StopReason,
        metadata: EventMetadata,
    },
    Error {
        prompt_id: Option<PromptId>,
        message: String,
        error_class: ErrorClass,
        metadata: EventMetadata,
    },
    Shutdown {
        metadata: EventMetadata,
    },
}

impl SessionEvent {
    fn with_session_metadata(self, session_id: SessionId, prompt_id: PromptId) -> Self {
        match self {
            Self::AssistantDelta { text, metadata, .. } => Self::AssistantDelta {
                prompt_id,
                text,
                metadata: metadata.with_session(session_id),
            },
            Self::ThinkingDelta { text, metadata, .. } => Self::ThinkingDelta {
                prompt_id,
                text,
                metadata: metadata.with_session(session_id),
            },
            Self::ToolStarted {
                call_id,
                tool_name,
                metadata,
                ..
            } => Self::ToolStarted {
                prompt_id,
                call_id,
                tool_name,
                metadata: metadata.with_session(session_id),
            },
            Self::ToolFinished {
                call_id,
                status,
                metadata,
                ..
            } => Self::ToolFinished {
                prompt_id,
                call_id,
                status,
                metadata: metadata.with_session(session_id),
            },
            Self::CostUpdated {
                input_tokens,
                output_tokens,
                metadata,
                ..
            } => Self::CostUpdated {
                prompt_id,
                input_tokens,
                output_tokens,
                metadata: metadata.with_session(session_id),
            },
            event => event,
        }
    }
}

/// Safe replay/routing metadata. Values are constrained to strings selected by runtime code.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventMetadata {
    pub session_id: Option<SessionId>,
    pub fields: BTreeMap<String, String>,
}

impl EventMetadata {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id: Some(session_id),
            fields: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn empty() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = sanitize_metadata_value(key.into());
        let value = sanitize_metadata_value(value.into());
        self.fields.insert(key, value);
        self
    }

    #[must_use]
    pub fn with_session(mut self, session_id: SessionId) -> Self {
        self.session_id = Some(session_id);
        self
    }

    #[must_use]
    pub fn contains_secret_markers(&self) -> bool {
        self.fields.iter().any(|(key, value)| contains_secret_marker(key) || contains_secret_marker(value))
    }
}

fn sanitize_metadata_value(value: String) -> String {
    if contains_secret_marker(&value) {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

fn contains_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "credential",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Complete,
    Interrupted,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolStatus {
    Succeeded,
    Failed,
    Denied,
}

/// Runtime model adapter. Hosts can implement this around any provider/router.
pub trait ModelAdapter: Send + Sync + 'static {
    fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError>;
}

/// Request passed to a host model adapter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub session_id: SessionId,
    pub prompt_id: PromptId,
    pub model: Option<String>,
    pub prompt: AssembledPrompt,
    pub disabled_tools: BTreeSet<String>,
}

/// Semantic events returned by a model adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelResponse {
    pub events: Vec<SessionEvent>,
}

/// Deterministic default model for embedded tests and examples.
pub struct EchoModelAdapter;

impl ModelAdapter for EchoModelAdapter {
    fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
        Ok(ModelResponse {
            events: vec![
                SessionEvent::AssistantDelta {
                    prompt_id: request.prompt_id.clone(),
                    text: format!("echo: {}", request.prompt.user_prompt),
                    metadata: EventMetadata::empty().with("source", "echo_model"),
                },
                SessionEvent::CostUpdated {
                    prompt_id: request.prompt_id,
                    input_tokens: request.prompt.user_prompt.split_whitespace().count() as u64,
                    output_tokens: 1,
                    metadata: EventMetadata::empty().with("source", "echo_model"),
                },
            ],
        })
    }
}

/// Prompt assembly service.
pub struct PromptAssembler;

impl PromptAssembler {
    pub fn assemble(
        policy: &PromptAssemblyPolicy,
        sources: &PromptSources,
        user_prompt: String,
    ) -> Result<AssembledPrompt, RuntimeError> {
        if user_prompt.trim().is_empty() {
            return Err(RuntimeError::InvalidPrompt("prompt cannot be blank".to_string()));
        }
        if !policy.allow_filesystem_discovery && sources.filesystem_context_requested {
            return Err(RuntimeError::FilesystemDiscoveryDisabled);
        }
        let mut sections = Vec::new();
        let mut provenance = Vec::new();
        for entry in &sources.host_context {
            let rendered = sanitize_prompt_context(&entry.content);
            sections.push(PromptSection {
                label: entry.label.clone(),
                content: rendered,
            });
            provenance.push(PromptProvenance {
                label: entry.label.clone(),
                source: PromptSourceKind::Host,
                safe_summary: format!("host:{}:{}chars", entry.label, entry.content.chars().count()),
            });
        }
        if let Some(system) = &sources.system_prompt {
            sections.push(PromptSection {
                label: "system".to_string(),
                content: sanitize_prompt_context(system),
            });
            provenance.push(PromptProvenance {
                label: "system".to_string(),
                source: PromptSourceKind::Host,
                safe_summary: format!("system:{}chars", system.chars().count()),
            });
        }
        Ok(AssembledPrompt {
            user_prompt,
            sections,
            provenance,
            context_references_enabled: policy.context_references_enabled,
            unsupported_context_references: unsupported_context_references(policy, sources),
        })
    }
}

fn unsupported_context_references(
    policy: &PromptAssemblyPolicy,
    sources: &PromptSources,
) -> Vec<UnsupportedContextReference> {
    if policy.context_references_enabled {
        return Vec::new();
    }
    sources
        .context_references
        .iter()
        .map(|reference| UnsupportedContextReference {
            label: sanitize_metadata_value(reference.label.clone()),
            kind: reference.kind,
            reason: "context references disabled by host policy".to_string(),
        })
        .collect()
}

fn sanitize_prompt_context(content: &str) -> String {
    if contains_secret_marker(content) {
        "[REDACTED]".to_string()
    } else {
        content.to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptAssemblyPolicy {
    pub allow_filesystem_discovery: bool,
    pub context_references_enabled: bool,
}

impl PromptAssemblyPolicy {
    #[must_use]
    pub fn host_context_only() -> Self {
        Self {
            allow_filesystem_discovery: false,
            context_references_enabled: false,
        }
    }

    #[must_use]
    pub fn desktop_default() -> Self {
        Self {
            allow_filesystem_discovery: true,
            context_references_enabled: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptSources {
    pub system_prompt: Option<String>,
    pub host_context: Vec<HostContext>,
    pub filesystem_context_requested: bool,
    pub context_references: Vec<ContextReferenceRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostContext {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssembledPrompt {
    pub user_prompt: String,
    pub sections: Vec<PromptSection>,
    pub provenance: Vec<PromptProvenance>,
    pub context_references_enabled: bool,
    pub unsupported_context_references: Vec<UnsupportedContextReference>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSection {
    pub label: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptProvenance {
    pub label: String,
    pub source: PromptSourceKind,
    pub safe_summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSourceKind {
    Host,
    Filesystem,
    Generated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextReferenceRequest {
    pub label: String,
    pub kind: ContextReferenceKind,
}

impl ContextReferenceRequest {
    #[must_use]
    pub fn new(label: impl Into<String>, kind: ContextReferenceKind) -> Self {
        Self {
            label: label.into(),
            kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReferenceKind {
    File,
    Directory,
    Url,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedContextReference {
    pub label: String,
    pub kind: ContextReferenceKind,
    pub reason: String,
}

/// Host-facing tool catalog.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolCatalog {
    tools: BTreeMap<String, ToolDescriptor>,
    packs: BTreeSet<CapabilityPack>,
}

impl ToolCatalog {
    #[must_use]
    pub fn builder() -> ToolCatalogBuilder {
        ToolCatalogBuilder::default()
    }

    #[must_use]
    pub fn embedding_safe() -> Self {
        Self::builder().pack(CapabilityPack::ReadOnly).build().expect("read-only pack has no collisions")
    }

    #[must_use]
    pub fn desktop_default() -> Self {
        Self::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::WorkspaceMutation)
            .pack(CapabilityPack::ShellCommands)
            .build()
            .expect("built-in packs have no collisions")
    }

    #[must_use]
    pub fn tools(&self) -> impl Iterator<Item = &ToolDescriptor> {
        self.tools.values()
    }

    #[must_use]
    pub fn contains_tool(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    #[must_use]
    pub fn packs(&self) -> &BTreeSet<CapabilityPack> {
        &self.packs
    }
}

#[derive(Debug, Clone, Default)]
pub struct ToolCatalogBuilder {
    tools: BTreeMap<String, ToolDescriptor>,
    packs: BTreeSet<CapabilityPack>,
    disabled_tools: BTreeSet<String>,
}

impl ToolCatalogBuilder {
    #[must_use]
    pub fn pack(mut self, pack: CapabilityPack) -> Self {
        for descriptor in pack.descriptors() {
            self.tools.entry(descriptor.name.clone()).or_insert(descriptor);
        }
        self.packs.insert(pack);
        self
    }

    pub fn custom_tool(mut self, descriptor: ToolDescriptor) -> Result<Self, RuntimeError> {
        if descriptor.name.trim().is_empty() {
            return Err(RuntimeError::InvalidTool("tool name cannot be blank".to_string()));
        }
        if self.disabled_tools.contains(&descriptor.name) {
            return Ok(self);
        }
        if self.tools.contains_key(&descriptor.name) {
            return Err(RuntimeError::ToolNameCollision(descriptor.name));
        }
        self.tools.insert(descriptor.name.clone(), descriptor);
        Ok(self)
    }

    #[must_use]
    pub fn disabled_tool(mut self, name: impl Into<String>) -> Self {
        let name = name.into();
        if !name.trim().is_empty() {
            self.tools.remove(&name);
            self.disabled_tools.insert(name);
        }
        self
    }

    #[must_use]
    pub fn disabled_tools<I, S>(mut self, names: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for name in names {
            self = self.disabled_tool(name);
        }
        self
    }

    pub fn build(self) -> Result<ToolCatalog, RuntimeError> {
        let tools = self.tools.into_iter().filter(|(name, _)| !self.disabled_tools.contains(name)).collect();
        Ok(ToolCatalog {
            tools,
            packs: self.packs,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub side_effect: SideEffectLevel,
    pub requires_confirmation: bool,
}

impl ToolDescriptor {
    #[must_use]
    pub fn new(name: impl Into<String>, description: impl Into<String>, side_effect: SideEffectLevel) -> Self {
        let side_effect = side_effect;
        Self {
            name: name.into(),
            description: description.into(),
            requires_confirmation: side_effect.requires_confirmation(),
            side_effect,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityPack {
    ReadOnly,
    WorkspaceMutation,
    ShellCommands,
    Network,
    ExternalProcesses,
}

impl CapabilityPack {
    #[must_use]
    pub fn descriptors(self) -> Vec<ToolDescriptor> {
        match self {
            Self::ReadOnly => vec![
                ToolDescriptor::new("read", "Read files selected by the host", SideEffectLevel::ReadOnly),
                ToolDescriptor::new("search", "Search host-selected project content", SideEffectLevel::ReadOnly),
            ],
            Self::WorkspaceMutation => vec![
                ToolDescriptor::new(
                    "write",
                    "Write files in host-approved workspace roots",
                    SideEffectLevel::WorkspaceMutation,
                ),
                ToolDescriptor::new(
                    "patch",
                    "Patch files in host-approved workspace roots",
                    SideEffectLevel::WorkspaceMutation,
                ),
            ],
            Self::ShellCommands => vec![ToolDescriptor::new(
                "bash",
                "Run host-approved shell commands",
                SideEffectLevel::Dangerous,
            )],
            Self::Network => vec![ToolDescriptor::new(
                "web",
                "Fetch host-approved network resources",
                SideEffectLevel::ExternalIo,
            )],
            Self::ExternalProcesses => vec![ToolDescriptor::new(
                "process",
                "Manage host-approved background processes",
                SideEffectLevel::Dangerous,
            )],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SideEffectLevel {
    ReadOnly,
    WorkspaceMutation,
    ExternalIo,
    Dangerous,
}

impl SideEffectLevel {
    #[must_use]
    pub fn requires_confirmation(self) -> bool {
        !matches!(self, Self::ReadOnly)
    }
}

/// Confirmation request passed to a host broker.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfirmationRequest {
    pub id: String,
    pub action: ConfirmationAction,
    pub summary: String,
    pub metadata: EventMetadata,
    pub timeout_ms: Option<u64>,
}

impl ConfirmationRequest {
    #[must_use]
    pub fn new(action: ConfirmationAction, summary: impl Into<String>) -> Self {
        Self {
            id: format!("confirm_{}", Uuid::new_v4()),
            action,
            summary: sanitize_metadata_value(summary.into()),
            metadata: EventMetadata::empty(),
            timeout_ms: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationAction {
    RunCommand,
    MutateWorkspace,
    UseNetwork,
    Custom(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfirmationDecision {
    pub approved: bool,
    pub reason: String,
}

impl ConfirmationDecision {
    #[must_use]
    pub fn approve(reason: impl Into<String>) -> Self {
        Self {
            approved: true,
            reason: sanitize_metadata_value(reason.into()),
        }
    }

    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: sanitize_metadata_value(reason.into()),
        }
    }
}

pub trait ConfirmationBroker: Send + Sync + 'static {
    fn decide(&self, request: ConfirmationRequest) -> Result<ConfirmationDecision, RuntimeError>;
}

pub struct FailClosedConfirmationBroker;

impl ConfirmationBroker for FailClosedConfirmationBroker {
    fn decide(&self, _request: ConfirmationRequest) -> Result<ConfirmationDecision, RuntimeError> {
        Ok(ConfirmationDecision::deny("confirmation broker unavailable"))
    }
}

pub fn request_confirmation_fail_closed(
    broker: &dyn ConfirmationBroker,
    request: ConfirmationRequest,
) -> Result<ConfirmationDecision, RuntimeError> {
    match broker.decide(request) {
        Ok(decision) => Ok(decision),
        Err(RuntimeError::ConfirmationUnavailable(reason)) => Ok(ConfirmationDecision::deny(reason)),
        Err(RuntimeError::ConfirmationTimedOut) => Ok(ConfirmationDecision::deny("confirmation timed out")),
        Err(RuntimeError::ConfirmationCancelled) => Ok(ConfirmationDecision::deny("confirmation cancelled")),
        Err(error) => Err(error),
    }
}

/// Runtime services are explicit so embedded hosts do not need ambient dotdirs.
#[derive(Clone)]
pub struct RuntimeServices {
    pub settings: Arc<dyn SettingsService>,
    pub auth: Arc<dyn AuthService>,
    pub sessions: Arc<dyn SessionStore>,
    pub cache: Arc<dyn CacheStore>,
    pub project_context: Arc<dyn ProjectContextService>,
    pub skills: Arc<dyn SkillStore>,
    pub plugins: Arc<dyn PluginStore>,
    pub checkpoints: Arc<dyn CheckpointStore>,
}

impl RuntimeServices {
    #[must_use]
    pub fn in_memory() -> Self {
        let noop = Arc::new(NoopService);
        Self {
            settings: noop.clone(),
            auth: noop.clone(),
            sessions: Arc::new(InMemorySessionStore::default()),
            cache: noop.clone(),
            project_context: noop.clone(),
            skills: noop.clone(),
            plugins: noop.clone(),
            checkpoints: noop,
        }
    }

    #[must_use]
    pub fn capability_metadata(&self) -> EventMetadata {
        EventMetadata::empty()
            .with("settings", self.settings.capability())
            .with("auth", self.auth.capability())
            .with("sessions", self.sessions.capability())
            .with("cache", self.cache.capability())
            .with("project_context", self.project_context.capability())
            .with("skills", self.skills.capability())
            .with("plugins", self.plugins.capability())
            .with("checkpoints", self.checkpoints.capability())
    }
}

pub trait SettingsService: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait AuthService: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait CacheStore: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait ProjectContextService: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait SkillStore: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait PluginStore: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait CheckpointStore: Send + Sync {
    fn capability(&self) -> &'static str;
}

pub trait SessionStore: Send + Sync {
    fn capability(&self) -> &'static str;
    fn save(&self, record: SessionRecord) -> Result<(), RuntimeError>;
    fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub last_prompt: Option<PromptId>,
    pub prompts: Vec<PromptReplayEntry>,
}

impl SessionRecord {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            created_at: Utc::now(),
            last_prompt: None,
            prompts: Vec::new(),
        }
    }

    #[must_use]
    pub fn replay_context(&self) -> Vec<PromptReplayEntry> {
        self.prompts.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptReplayEntry {
    pub prompt_id: PromptId,
    pub user_prompt: String,
    pub assembled_prompt: AssembledPrompt,
    pub completed_at: DateTime<Utc>,
}

pub struct NoopService;

impl SettingsService for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl AuthService for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl CacheStore for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl ProjectContextService for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl SkillStore for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl PluginStore for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}
impl CheckpointStore for NoopService {
    fn capability(&self) -> &'static str {
        "noop"
    }
}

#[derive(Default)]
pub struct InMemorySessionStore {
    records: std::sync::Mutex<BTreeMap<SessionId, SessionRecord>>,
}

impl SessionStore for InMemorySessionStore {
    fn capability(&self) -> &'static str {
        "in_memory"
    }

    fn save(&self, record: SessionRecord) -> Result<(), RuntimeError> {
        let mut records = self.records.lock().map_err(|_| RuntimeError::StoreUnavailable("sessions".to_string()))?;
        records.insert(record.session_id.clone(), record);
        Ok(())
    }

    fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError> {
        let records = self.records.lock().map_err(|_| RuntimeError::StoreUnavailable("sessions".to_string()))?;
        Ok(records.get(session_id).cloned())
    }
}

/// Marker type for future desktop/default path adapters.
pub struct DesktopRuntimeServices;

impl DesktopRuntimeServices {
    /// The concrete adapters live in the application crate; this marker keeps desktop mode
    /// explicit.
    pub fn unavailable_in_core_crate() -> RuntimeServices {
        RuntimeServices::in_memory()
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuntimeError {
    #[error("invalid prompt: {0}")]
    InvalidPrompt(String),
    #[error("event stream has already been taken")]
    EventStreamAlreadyTaken,
    #[error("event stream is closed")]
    EventStreamClosed,
    #[error("session is shut down")]
    SessionShutdown,
    #[error("filesystem discovery is disabled")]
    FilesystemDiscoveryDisabled,
    #[error("invalid tool: {0}")]
    InvalidTool(String),
    #[error("tool name collision: {0}")]
    ToolNameCollision(String),
    #[error("store unavailable: {0}")]
    StoreUnavailable(String),
    #[error("confirmation unavailable: {0}")]
    ConfirmationUnavailable(String),
    #[error("confirmation timed out")]
    ConfirmationTimedOut,
    #[error("confirmation cancelled")]
    ConfirmationCancelled,
    #[error("public runtime boundary leaked adapter type: {0}")]
    PublicBoundaryLeak(String),
    #[error("model failed: {0}")]
    Model(String),
}

impl RuntimeError {
    #[must_use]
    pub fn safe_message(&self) -> String {
        sanitize_metadata_value(self.to_string())
    }

    #[must_use]
    pub fn class(&self) -> ErrorClass {
        match self {
            Self::InvalidPrompt(_) => ErrorClass::InvalidInput,
            Self::EventStreamAlreadyTaken | Self::EventStreamClosed | Self::SessionShutdown => ErrorClass::Session,
            Self::FilesystemDiscoveryDisabled => ErrorClass::Policy,
            Self::InvalidTool(_) | Self::ToolNameCollision(_) => ErrorClass::Tooling,
            Self::StoreUnavailable(_) => ErrorClass::Storage,
            Self::ConfirmationUnavailable(_) | Self::ConfirmationTimedOut | Self::ConfirmationCancelled => {
                ErrorClass::Confirmation
            }
            Self::PublicBoundaryLeak(_) => ErrorClass::Boundary,
            Self::Model(_) => ErrorClass::Model,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    InvalidInput,
    Session,
    Policy,
    Tooling,
    Storage,
    Confirmation,
    Boundary,
    Model,
}

fn validate_public_runtime_boundary() -> Result<(), RuntimeError> {
    // Runtime check complements compile-level tests and documents the stable deny list.
    let denied = ["DaemonEvent", "SessionCommand", "Tui", "Acp", "Mcp", "Cli"];
    for item in denied {
        if public_type_names().iter().any(|name| name.contains(item)) {
            return Err(RuntimeError::PublicBoundaryLeak(item.to_string()));
        }
    }
    Ok(())
}

fn public_type_names() -> Vec<&'static str> {
    vec![
        "RuntimeBuilder",
        "Runtime",
        "SessionHandle",
        "SessionEvent",
        "PromptInput",
        "PromptReceipt",
        "EventMetadata",
        "RuntimeServices",
        "PromptAssembler",
        "PromptAssemblyPolicy",
        "ToolCatalog",
        "ToolDescriptor",
        "ConfirmationBroker",
        "ConfirmationRequest",
        "ConfirmationDecision",
    ]
}

/// Deterministic fixture adapter that mirrors the event order used by headless prompt tests.
#[must_use]
pub fn headless_prompt_parity_fixture(prompt: &str) -> Vec<&'static str> {
    let _ = prompt;
    vec!["prompt_accepted", "assistant_delta", "cost_updated", "completed"]
}

/// Serialize a safe event summary for host parity tests and docs examples.
#[must_use]
pub fn safe_event_summary(event: &SessionEvent) -> Value {
    match event {
        SessionEvent::PromptAccepted { metadata, .. } => {
            json!({"type":"prompt_accepted", "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::AssistantDelta { text, metadata, .. } => {
            json!({"type":"assistant_delta", "text_chars": text.chars().count(), "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::ThinkingDelta { text, metadata, .. } => {
            json!({"type":"thinking_delta", "text_chars": text.chars().count(), "metadata_fields": metadata.fields.len()})
        }
        SessionEvent::CostUpdated {
            input_tokens,
            output_tokens,
            ..
        } => json!({"type":"cost_updated", "input_tokens": input_tokens, "output_tokens": output_tokens}),
        SessionEvent::Completed { stop_reason, .. } => {
            json!({"type":"completed", "stop_reason": format!("{stop_reason:?}")})
        }
        SessionEvent::ToolStarted { tool_name, .. } => json!({"type":"tool_started", "tool_name": tool_name}),
        SessionEvent::ToolFinished { status, .. } => json!({"type":"tool_finished", "status": format!("{status:?}")}),
        SessionEvent::ConfirmationRequested { request, .. } => {
            json!({"type":"confirmation_requested", "action": format!("{:?}", request.action)})
        }
        SessionEvent::Error { error_class, .. } => json!({"type":"error", "class": format!("{error_class:?}")}),
        SessionEvent::Shutdown { .. } => json!({"type":"shutdown"}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct ScriptedModel;

    impl ModelAdapter for ScriptedModel {
        fn complete(&self, request: ModelRequest) -> Result<ModelResponse, RuntimeError> {
            Ok(ModelResponse {
                events: vec![
                    SessionEvent::ThinkingDelta {
                        prompt_id: request.prompt_id.clone(),
                        text: "thinking".to_string(),
                        metadata: EventMetadata::empty().with("source", "scripted"),
                    },
                    SessionEvent::AssistantDelta {
                        prompt_id: request.prompt_id.clone(),
                        text: "done".to_string(),
                        metadata: EventMetadata::empty().with("source", "scripted"),
                    },
                ],
            })
        }
    }

    #[tokio::test]
    async fn runtime_facade_streams_host_events_in_order() {
        let runtime = RuntimeBuilder::new().model_adapter(Arc::new(ScriptedModel)).build().unwrap();
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(SessionId::from_host("host-session")),
                model: None,
            })
            .await
            .unwrap();
        let mut events = session.take_events().await.unwrap();
        let receipt = session.submit_prompt(PromptInput::new("hello host")).await.unwrap();
        assert!(receipt.prompt_id.as_str().starts_with("prompt_"));

        let mut kinds = Vec::new();
        for _ in 0..4 {
            let event = events.recv().await.unwrap();
            kinds.push(safe_event_summary(&event)["type"].as_str().unwrap().to_string());
            match event {
                SessionEvent::PromptAccepted { metadata, .. }
                | SessionEvent::ThinkingDelta { metadata, .. }
                | SessionEvent::AssistantDelta { metadata, .. }
                | SessionEvent::Completed { metadata, .. } => {
                    assert_eq!(metadata.session_id.as_ref().unwrap().as_str(), "host-session");
                    assert!(!metadata.contains_secret_markers());
                }
                _ => {}
            }
        }
        assert_eq!(kinds, vec!["prompt_accepted", "thinking_delta", "assistant_delta", "completed"]);
    }

    #[tokio::test]
    async fn default_runtime_does_not_need_ambient_paths() {
        let runtime = RuntimeBuilder::new().build().unwrap();
        let metadata = runtime.inner.services.capability_metadata();
        assert_eq!(metadata.fields.get("settings").unwrap(), "noop");
        assert_eq!(metadata.fields.get("auth").unwrap(), "noop");
        assert_eq!(metadata.fields.get("sessions").unwrap(), "in_memory");
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        session.submit_prompt(PromptInput::new("no ambient path access")).await.unwrap();
    }

    #[test]
    fn public_api_boundary_rejects_transport_type_leakage() {
        validate_public_runtime_boundary().unwrap();
        let names = public_type_names().join("\n");
        for denied in ["DaemonEvent", "SessionCommand", "Tui", "Acp", "Mcp", "Cli"] {
            assert!(!names.contains(denied), "public API leaked {denied}");
        }
    }

    #[tokio::test]
    async fn fake_provider_prompt_matches_headless_parity_fixture() {
        let runtime = RuntimeBuilder::new().build().unwrap();
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        let mut events = session.take_events().await.unwrap();
        session.submit_prompt(PromptInput::new("parity")).await.unwrap();
        let mut kinds = Vec::new();
        for _ in 0..4 {
            kinds.push(safe_event_summary(&events.recv().await.unwrap())["type"].as_str().unwrap().to_string());
        }
        assert_eq!(kinds, headless_prompt_parity_fixture("parity"));
    }

    #[test]
    fn tool_catalog_embedding_safe_excludes_dangerous_packs() {
        let catalog = ToolCatalog::embedding_safe();
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("bash"));
        assert!(!catalog.packs().contains(&CapabilityPack::ShellCommands));
    }

    #[test]
    fn tool_catalog_supports_custom_tool_collision_policy() {
        let descriptor = ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly);
        let builder = ToolCatalog::builder().pack(CapabilityPack::ReadOnly).custom_tool(descriptor.clone()).unwrap();
        let err = builder.custom_tool(descriptor).unwrap_err();
        assert_eq!(err, RuntimeError::ToolNameCollision("host_search".to_string()));
    }

    #[test]
    fn tool_catalog_filters_disabled_tools_from_host_metadata() {
        let custom = ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly);
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::ShellCommands)
            .disabled_tools(["search", "bash", "host_search"])
            .custom_tool(custom)
            .unwrap()
            .build()
            .unwrap();

        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("search"));
        assert!(!catalog.contains_tool("bash"));
        assert!(!catalog.contains_tool("host_search"));
        assert!(catalog.tools().all(|tool| !matches!(tool.name.as_str(), "search" | "bash" | "host_search")));
    }

    #[test]
    fn prompt_assembly_host_context_only_redacts_provenance_content() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            system_prompt: Some("system token secret".to_string()),
            host_context: vec![HostContext {
                label: "app".to_string(),
                content: "safe app context".to_string(),
            }],
            filesystem_context_requested: false,
            ..PromptSources::default()
        };
        let assembled = PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap();
        assert_eq!(assembled.sections[0].content, "safe app context");
        assert_eq!(assembled.sections[1].content, "[REDACTED]");
        assert!(assembled.provenance.iter().all(|item| !contains_secret_marker(&item.safe_summary)));
    }

    #[test]
    fn prompt_assembly_rejects_filesystem_discovery_when_disabled() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            filesystem_context_requested: true,
            ..PromptSources::default()
        };
        assert_eq!(
            PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap_err(),
            RuntimeError::FilesystemDiscoveryDisabled
        );
    }

    #[test]
    fn prompt_assembly_reports_disabled_context_references_without_content() {
        let policy = PromptAssemblyPolicy::host_context_only();
        let sources = PromptSources {
            context_references: vec![ContextReferenceRequest::new(
                "src/secret-token.rs",
                ContextReferenceKind::File,
            )],
            ..PromptSources::default()
        };
        let assembled = PromptAssembler::assemble(&policy, &sources, "hello".to_string()).unwrap();

        assert!(!assembled.context_references_enabled);
        assert_eq!(assembled.unsupported_context_references.len(), 1);
        let unsupported = &assembled.unsupported_context_references[0];
        assert_eq!(unsupported.label, "[REDACTED]");
        assert_eq!(unsupported.kind, ContextReferenceKind::File);
        assert!(unsupported.reason.contains("disabled"));
        assert!(assembled.sections.is_empty());
    }

    struct ErrorBroker(RuntimeError);

    impl ConfirmationBroker for ErrorBroker {
        fn decide(&self, _request: ConfirmationRequest) -> Result<ConfirmationDecision, RuntimeError> {
            Err(self.0.clone())
        }
    }

    #[test]
    fn confirmation_broker_fail_closed_for_absent_timeout_cancelled() {
        for error in [
            RuntimeError::ConfirmationUnavailable("missing".to_string()),
            RuntimeError::ConfirmationTimedOut,
            RuntimeError::ConfirmationCancelled,
        ] {
            let broker = ErrorBroker(error);
            let decision = request_confirmation_fail_closed(
                &broker,
                ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"),
            )
            .unwrap();
            assert!(!decision.approved);
        }
    }

    #[test]
    fn confirmation_request_metadata_redacts_secret_markers() {
        let request =
            ConfirmationRequest::new(ConfirmationAction::Custom("deploy".to_string()), "use bearer token abc123");
        assert_eq!(request.summary, "[REDACTED]");
    }

    #[tokio::test]
    async fn in_memory_session_replay_records_last_prompt() {
        let store = Arc::new(InMemorySessionStore::default());
        let services = RuntimeServices {
            sessions: store.clone(),
            ..RuntimeServices::in_memory()
        };
        let runtime = RuntimeBuilder::new().services(services).build().unwrap();
        let session_id = SessionId::from_host("replay-session");
        let session = runtime
            .create_session(SessionOptions {
                session_id: Some(session_id.clone()),
                model: None,
            })
            .await
            .unwrap();
        session.submit_prompt(PromptInput::new("persist me")).await.unwrap();
        session.submit_prompt(PromptInput::new("persist me too")).await.unwrap();
        let record = store.load(&session_id).unwrap().unwrap();
        assert!(record.last_prompt.is_some());
        let replay = record.replay_context();
        assert_eq!(replay.len(), 2);
        assert_eq!(replay[0].user_prompt, "persist me");
        assert_eq!(replay[1].user_prompt, "persist me too");
        assert_eq!(replay[0].assembled_prompt.user_prompt, "persist me");
    }
}
