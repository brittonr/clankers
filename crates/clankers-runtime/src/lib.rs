//! Host-facing embedding facade for Clankers.
//!
//! This crate is intentionally transport-neutral. Public types model sessions,
//! prompts, tools, confirmation decisions, prompt assembly, and runtime-owned
//! services without exposing daemon frames, TUI state, CLI arguments, ACP/MCP
//! envelopes, or Matrix adapter types.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
#[cfg(test)]
use serde_json::json;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use uuid::Uuid;

mod boundary;
mod event_summary;
pub mod events;
pub mod prompt;
pub mod tools;

#[cfg(test)]
use boundary::public_type_names;
use boundary::validate_public_runtime_boundary;
pub use event_summary::headless_prompt_parity_fixture;
pub use event_summary::safe_event_summary;
pub use events::ErrorClass;
pub use events::EventMetadata;
pub use events::SessionEvent;
pub use events::StopReason;
pub use events::ToolStatus;
use events::contains_secret_marker;
use events::sanitize_metadata_value;
pub use prompt::AssembledPrompt;
pub use prompt::ContextReferenceKind;
pub use prompt::ContextReferenceRequest;
pub use prompt::EchoModelAdapter;
pub use prompt::HostContext;
pub use prompt::ModelAdapter;
pub use prompt::ModelRequest;
pub use prompt::ModelResponse;
pub use prompt::PromptAssembler;
pub use prompt::PromptAssemblyPolicy;
pub use prompt::PromptId;
pub use prompt::PromptInput;
pub use prompt::PromptProvenance;
pub use prompt::PromptReceipt;
pub use prompt::PromptSection;
pub use prompt::PromptSourceKind;
pub use prompt::PromptSources;
pub use prompt::UnsupportedContextReference;
pub use tools::CapabilityPack;
pub use tools::SideEffectLevel;
pub use tools::ToolCatalog;
pub use tools::ToolCatalogBuilder;
pub use tools::ToolCatalogOmission;
pub use tools::ToolCollisionPolicy;
pub use tools::ToolDescriptor;

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
        request_confirmation_fail_closed(self.inner.confirmation_broker.as_ref(), request).await
    }

    /// Execute a host action only after the broker approves the typed request.
    pub async fn run_confirmed_action<T>(
        &self,
        request: ConfirmationRequest,
        action: impl FnOnce() -> Result<T, RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let decision = self.request_confirmation(request).await?;
        if !decision.approved {
            return Err(RuntimeError::ConfirmationDenied(decision.reason));
        }
        action()
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

pub type ConfirmationFuture<'a> = Pin<Box<dyn Future<Output = Result<ConfirmationDecision, RuntimeError>> + Send + 'a>>;

pub trait ConfirmationBroker: Send + Sync + 'static {
    fn decide(&self, request: ConfirmationRequest) -> ConfirmationFuture<'_>;
}

pub struct FailClosedConfirmationBroker;

impl ConfirmationBroker for FailClosedConfirmationBroker {
    fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
        Box::pin(async { Ok(ConfirmationDecision::deny("confirmation broker unavailable")) })
    }
}

pub async fn request_confirmation_fail_closed(
    broker: &dyn ConfirmationBroker,
    request: ConfirmationRequest,
) -> Result<ConfirmationDecision, RuntimeError> {
    match broker.decide(request).await {
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
    pub extensions: ExtensionServices,
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
            extensions: ExtensionServices::disabled(),
        }
    }

    #[must_use]
    pub fn capability_metadata(&self) -> EventMetadata {
        let extension_metadata = self.extensions.capability_metadata();
        EventMetadata::empty()
            .with("settings", self.settings.capability())
            .with("auth", self.auth.capability())
            .with("sessions", self.sessions.capability())
            .with("cache", self.cache.capability())
            .with("project_context", self.project_context.capability())
            .with("skills", self.skills.capability())
            .with("plugins", self.plugins.capability())
            .with("checkpoints", self.checkpoints.capability())
            .with(
                "provider_router",
                extension_metadata.fields.get("provider_router").cloned().unwrap_or_else(|| "disabled".to_string()),
            )
            .with(
                "extension_auth_store",
                extension_metadata.fields.get("auth_store").cloned().unwrap_or_else(|| "disabled".to_string()),
            )
            .with(
                "credential_pool",
                extension_metadata.fields.get("credential_pool").cloned().unwrap_or_else(|| "disabled".to_string()),
            )
            .with(
                "extension_runtime",
                extension_metadata.fields.get("runtime").cloned().unwrap_or_else(|| "disabled".to_string()),
            )
    }
}

/// Host-owned extension services for side-effectful provider/router/auth/plugin/MCP/gateway
/// systems.
#[derive(Clone)]
pub struct ExtensionServices {
    pub provider_router: Arc<dyn ProviderRouterService>,
    pub auth_store: Arc<dyn ExtensionAuthStoreService>,
    pub credential_pool: Arc<dyn CredentialPoolPolicyService>,
    pub runtime: Arc<dyn ExtensionRuntimeService>,
}

impl ExtensionServices {
    #[must_use]
    pub fn disabled() -> Self {
        let disabled = Arc::new(DisabledExtensionService);
        Self {
            provider_router: disabled.clone(),
            auth_store: disabled.clone(),
            credential_pool: disabled.clone(),
            runtime: disabled,
        }
    }

    #[must_use]
    pub fn capability_metadata(&self) -> EventMetadata {
        EventMetadata::empty()
            .with("provider_router", self.provider_router.capability())
            .with("auth_store", self.auth_store.capability())
            .with("credential_pool", self.credential_pool.capability())
            .with("runtime", self.runtime.capability())
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

pub trait ProviderRouterService: Send + Sync {
    fn capability(&self) -> &'static str;
    fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError>;
}

pub trait ExtensionAuthStoreService: Send + Sync {
    fn capability(&self) -> &'static str;
    fn access(&self, request: AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError>;
}

pub trait CredentialPoolPolicyService: Send + Sync {
    fn capability(&self) -> &'static str;
    fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError>;
}

pub trait ExtensionRuntimeService: Send + Sync {
    fn capability(&self) -> &'static str;
    fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError>;
    fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderExecutionRequest {
    pub provider: String,
    pub model: Option<String>,
    pub account_label: Option<String>,
    pub route_source: String,
    pub prompt: Option<String>,
    pub system_prompt: Option<String>,
    pub max_tokens: Option<usize>,
    pub session_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStoreOperation {
    Lookup,
    RefreshPersist,
    PendingLoginVerifier,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthStoreAccessRequest {
    pub provider: String,
    pub account_label: Option<String>,
    pub operation: AuthStoreOperation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPoolRequest {
    pub provider: String,
    pub strategy: String,
    pub account_label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntimeKind {
    Plugin,
    Mcp,
    Gateway,
}

fn extension_kind_label(kind: ExtensionRuntimeKind) -> &'static str {
    match kind {
        ExtensionRuntimeKind::Plugin => "plugin",
        ExtensionRuntimeKind::Mcp => "mcp",
        ExtensionRuntimeKind::Gateway => "gateway",
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionRuntimeRequest {
    pub kind: ExtensionRuntimeKind,
    pub action: String,
    pub extension_name: Option<String>,
    pub visible_tool_name: Option<String>,
    pub original_tool_name: Option<String>,
    pub runtime_entrypoint: Option<String>,
    #[serde(default)]
    pub arguments: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionToolDescriptor {
    pub source: ExtensionRuntimeKind,
    pub visible_tool_name: String,
    pub original_tool_name: Option<String>,
    pub side_effect: SideEffectLevel,
    pub prerequisites: Vec<String>,
    pub metadata: EventMetadata,
}

impl ExtensionToolDescriptor {
    #[must_use]
    pub fn new(
        source: ExtensionRuntimeKind,
        visible_tool_name: impl Into<String>,
        original_tool_name: Option<String>,
        side_effect: SideEffectLevel,
    ) -> Self {
        Self {
            source,
            visible_tool_name: sanitize_metadata_value(visible_tool_name.into()),
            original_tool_name: original_tool_name.map(sanitize_metadata_value),
            side_effect,
            prerequisites: Vec::new(),
            metadata: EventMetadata::empty().with("source", format!("{source:?}")),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata = self.metadata.with(key, value);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    Succeeded,
    Failed,
    Disabled,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExtensionReceipt {
    pub source: String,
    pub action: String,
    pub status: ExtensionStatus,
    pub duration_ms: Option<u64>,
    pub error_class: Option<ErrorClass>,
    pub metadata: EventMetadata,
}

impl ExtensionReceipt {
    #[must_use]
    pub fn new(source: impl Into<String>, action: impl Into<String>, status: ExtensionStatus) -> Self {
        Self {
            source: sanitize_metadata_value(source.into()),
            action: sanitize_metadata_value(action.into()),
            status,
            duration_ms: None,
            error_class: None,
            metadata: EventMetadata::empty(),
        }
    }

    #[must_use]
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata = self.metadata.with(key, value);
        self
    }

    #[must_use]
    pub fn with_error_class(mut self, class: ErrorClass) -> Self {
        self.error_class = Some(class);
        self
    }

    #[must_use]
    pub fn contains_secret_markers(&self) -> bool {
        contains_secret_marker(&self.source)
            || contains_secret_marker(&self.action)
            || self.metadata.contains_secret_markers()
    }
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

pub struct DisabledExtensionService;

impl ProviderRouterService for DisabledExtensionService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("provider router disabled".to_string()))
    }
}

impl ExtensionAuthStoreService for DisabledExtensionService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn access(&self, request: AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("extension auth store disabled".to_string()))
    }
}

impl CredentialPoolPolicyService for DisabledExtensionService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("credential pool disabled".to_string()))
    }
}

impl ExtensionRuntimeService for DisabledExtensionService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
        let _ = kind;
        Ok(Vec::new())
    }

    fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("extension runtime disabled".to_string()))
    }
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
    #[error("confirmation denied: {0}")]
    ConfirmationDenied(String),
    #[error("extension unavailable: {0}")]
    ExtensionUnavailable(String),
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
            Self::ConfirmationUnavailable(_)
            | Self::ConfirmationTimedOut
            | Self::ConfirmationCancelled
            | Self::ConfirmationDenied(_) => ErrorClass::Confirmation,
            Self::ExtensionUnavailable(_) => ErrorClass::Extension,
            Self::PublicBoundaryLeak(_) => ErrorClass::Boundary,
            Self::Model(_) => ErrorClass::Model,
        }
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
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("extension_auth_store").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "disabled");
        assert_eq!(metadata.fields.get("extension_runtime").unwrap(), "disabled");
        let session = runtime.create_session(SessionOptions::default()).await.unwrap();
        session.submit_prompt(PromptInput::new("no ambient path access")).await.unwrap();
    }

    #[test]
    fn disabled_extension_services_fail_closed_without_startup_side_effects() {
        let extensions = ExtensionServices::disabled();
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap(), Vec::new());
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap(), Vec::new());
        assert_eq!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Gateway).unwrap(), Vec::new());

        let provider_error = extensions
            .provider_router
            .execute(ProviderExecutionRequest {
                provider: "openai-codex".to_string(),
                model: Some("gpt-5.3-codex".to_string()),
                account_label: Some("desktop".to_string()),
                route_source: "embedded".to_string(),
                prompt: Some("hello".to_string()),
                system_prompt: None,
                max_tokens: Some(8),
                session_id: Some("session-runtime-test".to_string()),
            })
            .unwrap_err();
        assert_eq!(provider_error, RuntimeError::ExtensionUnavailable("provider router disabled".to_string()));

        let auth_error = extensions
            .auth_store
            .access(AuthStoreAccessRequest {
                provider: "anthropic".to_string(),
                account_label: Some("default".to_string()),
                operation: AuthStoreOperation::PendingLoginVerifier,
            })
            .unwrap_err();
        assert_eq!(auth_error, RuntimeError::ExtensionUnavailable("extension auth store disabled".to_string()));

        let pool_error = extensions
            .credential_pool
            .select(CredentialPoolRequest {
                provider: "anthropic".to_string(),
                strategy: "fill_first".to_string(),
                account_label: None,
            })
            .unwrap_err();
        assert_eq!(pool_error, RuntimeError::ExtensionUnavailable("credential pool disabled".to_string()));

        let runtime_error = extensions
            .runtime
            .execute(ExtensionRuntimeRequest {
                kind: ExtensionRuntimeKind::Plugin,
                action: "call".to_string(),
                extension_name: Some("plugin_secret_token_runtime".to_string()),
                visible_tool_name: Some("plugin_secret_token_tool".to_string()),
                original_tool_name: Some("raw".to_string()),
                runtime_entrypoint: Some("handle_tool_call".to_string()),
                arguments: json!({"secret_token": "abc123"}),
            })
            .unwrap_err();
        assert_eq!(runtime_error, RuntimeError::ExtensionUnavailable("extension runtime disabled".to_string()));
    }

    #[test]
    fn extension_receipts_and_descriptors_redact_secret_like_metadata() {
        let receipt =
            ExtensionReceipt::new("bearer token provider", "authorization header call", ExtensionStatus::Failed)
                .with_metadata("api_key", "abc123")
                .with_metadata("provider", "anthropic")
                .with_error_class(ErrorClass::Extension);
        assert_eq!(receipt.source, "[REDACTED]");
        assert_eq!(receipt.action, "[REDACTED]");
        assert_eq!(receipt.metadata.fields.get("api_key").unwrap(), "[REDACTED]");
        assert_eq!(receipt.metadata.fields.get("provider").unwrap(), "anthropic");
        assert!(!receipt.contains_secret_markers());

        let descriptor = ExtensionToolDescriptor::new(
            ExtensionRuntimeKind::Mcp,
            "mcp_authorization_header_tool",
            Some("plugin token payload".to_string()),
            SideEffectLevel::ExternalIo,
        );
        assert_eq!(descriptor.visible_tool_name, "[REDACTED]");
        assert_eq!(descriptor.original_tool_name.as_deref(), Some("[REDACTED]"));
        assert!(!descriptor.metadata.contains_secret_markers());
    }

    struct StaticExtensionService;

    impl ProviderRouterService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_provider_router"
        }

        fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_provider_router", "execute", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source))
        }
    }

    impl ExtensionAuthStoreService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_auth_store"
        }

        fn access(&self, request: AuthStoreAccessRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(
                ExtensionReceipt::new(
                    "host_auth_store",
                    format!("{:?}", request.operation),
                    ExtensionStatus::Succeeded,
                )
                .with_metadata("provider", request.provider),
            )
        }
    }

    impl CredentialPoolPolicyService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_credential_pool"
        }

        fn select(&self, request: CredentialPoolRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_credential_pool", request.strategy, ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider))
        }
    }

    impl ExtensionRuntimeService for StaticExtensionService {
        fn capability(&self) -> &'static str {
            "host_extension_runtime"
        }

        fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
            Ok(vec![ExtensionToolDescriptor::new(
                kind,
                "host_visible_tool",
                Some("original_tool".to_string()),
                SideEffectLevel::ExternalIo,
            )])
        }

        fn execute(&self, request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
            Ok(ExtensionReceipt::new("host_extension_runtime", request.action, ExtensionStatus::Succeeded))
        }
    }

    #[test]
    fn host_supplied_extension_services_are_explicit_capabilities() {
        let service = Arc::new(StaticExtensionService);
        let extensions = ExtensionServices {
            provider_router: service.clone(),
            auth_store: service.clone(),
            credential_pool: service.clone(),
            runtime: service,
        };
        let metadata = extensions.capability_metadata();
        assert_eq!(metadata.fields.get("provider_router").unwrap(), "host_provider_router");
        assert_eq!(metadata.fields.get("auth_store").unwrap(), "host_auth_store");
        assert_eq!(metadata.fields.get("credential_pool").unwrap(), "host_credential_pool");
        assert_eq!(metadata.fields.get("runtime").unwrap(), "host_extension_runtime");

        let tools = extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].visible_tool_name, "host_visible_tool");
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

    #[derive(Default)]
    struct CountingExtensionRuntimeService {
        publish_calls: std::sync::atomic::AtomicUsize,
        execute_calls: std::sync::atomic::AtomicUsize,
    }

    impl ExtensionRuntimeService for CountingExtensionRuntimeService {
        fn capability(&self) -> &'static str {
            "counting_extension_runtime"
        }

        fn publishable_tools(&self, kind: ExtensionRuntimeKind) -> Result<Vec<ExtensionToolDescriptor>, RuntimeError> {
            self.publish_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![ExtensionToolDescriptor::new(
                kind,
                "plugin_echo",
                Some("echo".to_string()),
                SideEffectLevel::ExternalIo,
            )])
        }

        fn execute(&self, _request: ExtensionRuntimeRequest) -> Result<ExtensionReceipt, RuntimeError> {
            self.execute_calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ExtensionReceipt::new("counting_extension_runtime", "execute", ExtensionStatus::Succeeded))
        }
    }

    #[test]
    fn tool_catalog_capability_pack_matrix_does_not_expand_dangerous_packs() {
        let cases = [
            (vec![CapabilityPack::ReadOnly], vec!["read", "search"], vec!["write", "bash", "web", "process"]),
            (
                vec![CapabilityPack::ReadOnly, CapabilityPack::WorkspaceMutation],
                vec!["read", "write", "patch"],
                vec!["bash", "web", "process"],
            ),
            (vec![CapabilityPack::ShellCommands], vec!["bash"], vec!["web", "process"]),
            (vec![CapabilityPack::Network], vec!["web"], vec!["bash", "process"]),
            (vec![CapabilityPack::ExternalProcesses], vec!["process"], vec!["bash", "web"]),
        ];
        for (packs, included, excluded) in cases {
            let mut builder = ToolCatalog::builder();
            for pack in packs {
                builder = builder.pack(pack);
            }
            let catalog = builder.build().unwrap();
            for name in included {
                assert!(catalog.contains_tool(name), "expected tool {name}");
            }
            for name in excluded {
                assert!(!catalog.contains_tool(name), "unexpected tool {name}");
            }
            for descriptor in catalog.tools() {
                assert_eq!(descriptor.requires_confirmation, descriptor.side_effect.requires_confirmation());
            }
        }
    }

    #[test]
    fn tool_catalog_disabled_filter_overrides_packs_with_safe_omissions() {
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .pack(CapabilityPack::ShellCommands)
            .disabled_tools(["search", "bash"])
            .build()
            .unwrap();
        assert!(catalog.contains_tool("read"));
        assert!(!catalog.contains_tool("search"));
        assert!(!catalog.contains_tool("bash"));
        assert!(
            catalog
                .omissions()
                .iter()
                .any(|item| item.name == "search" && item.reason == "disabled_by_host_filter")
        );
        assert!(
            catalog
                .omissions()
                .iter()
                .any(|item| item.name == "bash" && item.reason == "disabled_by_host_filter")
        );
        let serialized = serde_json::to_string(catalog.omissions()).unwrap();
        assert!(!serialized.contains("TOKEN"));
        assert!(!serialized.contains("api_key"));
    }

    #[test]
    fn tool_catalog_custom_tools_apply_collision_policy_matrix() {
        let host_search =
            ToolDescriptor::new("host_search", "host search", SideEffectLevel::ReadOnly).with_source("host");
        let catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .custom_tool(host_search)
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(catalog.tools().find(|tool| tool.name == "host_search").unwrap().source, "host");

        let collision =
            ToolDescriptor::new("read", "host read", SideEffectLevel::WorkspaceMutation).with_source("host");
        let err = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::Reject)
            .custom_tool(collision.clone())
            .unwrap_err();
        assert_eq!(err, RuntimeError::ToolNameCollision("read".to_string()));

        let keep = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::KeepExisting)
            .custom_tool(collision.clone())
            .unwrap()
            .build()
            .unwrap();
        assert_eq!(keep.tools().find(|tool| tool.name == "read").unwrap().source, "clankers");
        assert!(keep.omissions().iter().any(|item| item.reason == "name_collision_existing_kept"));

        let override_catalog = ToolCatalog::builder()
            .pack(CapabilityPack::ReadOnly)
            .collision_policy(ToolCollisionPolicy::HostOverrides)
            .custom_tool(collision)
            .unwrap()
            .build()
            .unwrap();
        let read = override_catalog.tools().find(|tool| tool.name == "read").unwrap();
        assert_eq!(read.source, "host");
        assert_eq!(read.side_effect, SideEffectLevel::WorkspaceMutation);
    }

    #[test]
    fn tool_catalog_extension_descriptors_require_runtime_availability_without_execute() {
        let disabled = ExtensionServices::disabled();
        let disabled_catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, disabled.runtime.as_ref())
            .unwrap()
            .build()
            .unwrap();
        assert!(!disabled_catalog.contains_tool("plugin_echo"));

        let runtime = CountingExtensionRuntimeService::default();
        let catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, &runtime)
            .unwrap()
            .build()
            .unwrap();
        let tool = catalog.tools().find(|tool| tool.name == "plugin_echo").unwrap();
        assert_eq!(tool.source, "extension:plugin");
        assert_eq!(tool.side_effect, SideEffectLevel::ExternalIo);
        assert_eq!(runtime.publish_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn tool_catalog_metadata_query_does_not_start_extension_runtimes() {
        let runtime = CountingExtensionRuntimeService::default();
        let catalog = ToolCatalog::builder()
            .extension_runtime_tools(ExtensionRuntimeKind::Plugin, &runtime)
            .unwrap()
            .build()
            .unwrap();
        let _ = catalog.tools().collect::<Vec<_>>();
        assert!(catalog.contains_tool("plugin_echo"));
        assert!(catalog.omissions().is_empty());
        assert_eq!(runtime.publish_calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert_eq!(runtime.execute_calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[test]
    fn runtime_extension_service_matrix_default_safe_fails_closed_independently() {
        let extensions = ExtensionServices::disabled();
        assert!(matches!(
            extensions.provider_router.execute(provider_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(matches!(extensions.auth_store.access(auth_request()), Err(RuntimeError::ExtensionUnavailable(_))));
        assert!(matches!(
            extensions.credential_pool.select(pool_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap().is_empty());
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Mcp).unwrap().is_empty());
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Gateway).unwrap().is_empty());
        assert!(matches!(extensions.runtime.execute(runtime_request()), Err(RuntimeError::ExtensionUnavailable(_))));
    }

    fn provider_request() -> ProviderExecutionRequest {
        ProviderExecutionRequest {
            provider: "anthropic".to_string(),
            model: Some("test".to_string()),
            account_label: Some("primary".to_string()),
            route_source: "embedded".to_string(),
            prompt: Some("hello".to_string()),
            system_prompt: None,
            max_tokens: Some(8),
            session_id: Some("session".to_string()),
        }
    }

    fn auth_request() -> AuthStoreAccessRequest {
        AuthStoreAccessRequest {
            provider: "anthropic".to_string(),
            account_label: Some("primary".to_string()),
            operation: AuthStoreOperation::Lookup,
        }
    }

    fn pool_request() -> CredentialPoolRequest {
        CredentialPoolRequest {
            provider: "anthropic".to_string(),
            strategy: "fill_first".to_string(),
            account_label: Some("primary".to_string()),
        }
    }

    fn runtime_request() -> ExtensionRuntimeRequest {
        ExtensionRuntimeRequest {
            kind: ExtensionRuntimeKind::Plugin,
            action: "execute".to_string(),
            extension_name: Some("plugin".to_string()),
            visible_tool_name: Some("tool".to_string()),
            original_tool_name: Some("tool".to_string()),
            runtime_entrypoint: Some("main".to_string()),
            arguments: json!({}),
        }
    }

    #[derive(Default)]
    struct CountingProviderRouterService(std::sync::atomic::AtomicUsize);

    impl ProviderRouterService for CountingProviderRouterService {
        fn capability(&self) -> &'static str {
            "injected_provider_router"
        }
        fn execute(&self, request: ProviderExecutionRequest) -> Result<ExtensionReceipt, RuntimeError> {
            self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(ExtensionReceipt::new("injected_provider_router", "execute", ExtensionStatus::Succeeded)
                .with_metadata("provider", request.provider)
                .with_metadata("route_source", request.route_source))
        }
    }

    #[test]
    fn runtime_extension_service_matrix_mixed_injected_absent_no_ambient_fallback() {
        let provider = Arc::new(CountingProviderRouterService::default());
        let extensions = ExtensionServices {
            provider_router: provider.clone(),
            auth_store: Arc::new(DisabledExtensionService),
            credential_pool: Arc::new(DisabledExtensionService),
            runtime: Arc::new(DisabledExtensionService),
        };
        let receipt = extensions.provider_router.execute(provider_request()).unwrap();
        assert_eq!(receipt.status, ExtensionStatus::Succeeded);
        assert!(matches!(extensions.auth_store.access(auth_request()), Err(RuntimeError::ExtensionUnavailable(_))));
        assert!(matches!(
            extensions.credential_pool.select(pool_request()),
            Err(RuntimeError::ExtensionUnavailable(_))
        ));
        assert!(extensions.runtime.publishable_tools(ExtensionRuntimeKind::Plugin).unwrap().is_empty());
        assert_eq!(provider.0.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn runtime_extension_service_matrix_injected_error_receipts_are_redacted() {
        let receipt = ExtensionReceipt::new("provider_router", "execute", ExtensionStatus::Failed)
            .with_metadata("api_key", "secret-token")
            .with_metadata("prompt_bytes", "123")
            .with_error_class(ErrorClass::Extension);
        let serialized = serde_json::to_string(&receipt).unwrap();
        assert!(!receipt.contains_secret_markers());
        assert!(!serialized.contains("secret-token"));
        assert!(serialized.contains("prompt_bytes"));
    }

    #[test]
    fn runtime_extension_service_matrix_safe_receipts_redact_success_denial_and_error() {
        for status in [
            ExtensionStatus::Succeeded,
            ExtensionStatus::Unavailable,
            ExtensionStatus::Failed,
        ] {
            let receipt = ExtensionReceipt::new("provider", "execute", status)
                .with_metadata("provider", "anthropic")
                .with_metadata("output_bytes", "42")
                .with_metadata("bearer_token", "secret");
            let serialized = serde_json::to_string(&receipt).unwrap();
            assert!(!receipt.contains_secret_markers());
            assert!(serialized.contains("anthropic"));
            assert!(serialized.contains("42"));
            assert!(!serialized.contains("secret"));
        }
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
        fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
            let error = self.0.clone();
            Box::pin(async move { Err(error) })
        }
    }

    struct StaticBroker(ConfirmationDecision);

    impl ConfirmationBroker for StaticBroker {
        fn decide(&self, _request: ConfirmationRequest) -> ConfirmationFuture<'_> {
            let decision = self.0.clone();
            Box::pin(async move { Ok(decision) })
        }
    }

    #[tokio::test]
    async fn confirmation_broker_fail_closed_for_absent_timeout_cancelled() {
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
            .await
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
    async fn confirmed_action_does_not_execute_before_approval() {
        let denied_runtime = RuntimeBuilder::new()
            .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::deny("no"))))
            .build()
            .unwrap();
        let mut executed = false;
        let err = denied_runtime
            .run_confirmed_action(ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"), || {
                executed = true;
                Ok(())
            })
            .await
            .unwrap_err();
        assert!(!executed);
        assert_eq!(err, RuntimeError::ConfirmationDenied("no".to_string()));

        let approved_runtime = RuntimeBuilder::new()
            .confirmation_broker(Arc::new(StaticBroker(ConfirmationDecision::approve("yes"))))
            .build()
            .unwrap();
        let mut approved_executed = false;
        approved_runtime
            .run_confirmed_action(ConfirmationRequest::new(ConfirmationAction::RunCommand, "run command"), || {
                approved_executed = true;
                Ok(())
            })
            .await
            .unwrap();
        assert!(approved_executed);
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
