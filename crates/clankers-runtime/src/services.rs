//! Host-owned runtime service contracts and default-safe adapters.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::DateTime;
use chrono::Utc;
pub use clanker_message::AuthStoreAccessRequest;
pub use clanker_message::AuthStoreOperation;
use clanker_message::Content;
pub use clanker_message::CredentialPoolRequest;
pub use clanker_message::ExtensionRuntimeKind;
pub use clanker_message::ExtensionRuntimeRequest;
pub use clanker_message::ExtensionStatus;
pub use clanker_message::ProviderMessage;
pub use clanker_message::ProviderMessageRole;
pub use clanker_message::ProviderModelFailure;
pub use clanker_message::ProviderModelStatus;
pub use clanker_message::ProviderStreamEvent;
pub use clanker_message::ResolvedSkillSnippet;
pub use clanker_message::SkillResolutionRequest;
use clanker_message::StopReason;
use clanker_message::ThinkingConfig;
use clanker_message::ToolDefinition;
use clanker_message::Usage;
use serde::Deserialize;
use serde::Serialize;

use crate::AssembledPrompt;
use crate::ErrorClass;
use crate::EventMetadata;
use crate::PromptId;
use crate::RuntimeError;
use crate::SessionId;
use crate::SessionLedgerEntry;
use crate::SideEffectLevel;
use crate::events::contains_secret_marker;
use crate::events::sanitize_metadata_value;

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
    pub fn stateless() -> Self {
        let noop = Arc::new(NoopService);
        Self {
            settings: noop.clone(),
            auth: noop.clone(),
            sessions: Arc::new(DisabledSessionStore),
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

    fn resolve(&self, request: SkillResolutionRequest) -> Result<SkillResolution, RuntimeError> {
        let _ = request;
        Err(RuntimeError::ExtensionUnavailable("skill service unavailable".to_string()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillResolution {
    pub snippets: Vec<ResolvedSkillSnippet>,
    pub receipt: ExtensionReceipt,
}
pub trait PluginStore: Send + Sync {
    fn capability(&self) -> &'static str;
}
pub trait CheckpointStore: Send + Sync {
    fn capability(&self) -> &'static str;
}

pub trait ProviderRouterService: Send + Sync {
    fn capability(&self) -> &'static str;
    fn complete(&self, request: ProviderModelRequest) -> Result<ProviderModelResponse, RuntimeError>;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelRequest {
    pub provider: String,
    pub model: Option<String>,
    pub account_label: Option<String>,
    pub route_source: String,
    pub session_id: Option<String>,
    pub system_prompt: Option<String>,
    pub messages: Vec<ProviderMessage>,
    pub tools: Vec<ToolDefinition>,
    pub thinking: Option<ThinkingConfig>,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
    pub metadata: EventMetadata,
}

impl ProviderModelRequest {
    #[must_use]
    pub fn user_prompt(provider: impl Into<String>, model: Option<String>, prompt: impl Into<String>) -> Self {
        Self {
            provider: sanitize_metadata_value(provider.into()),
            model: model.map(sanitize_metadata_value),
            account_label: None,
            route_source: "runtime".to_string(),
            session_id: None,
            system_prompt: None,
            messages: vec![ProviderMessage::user_text(prompt)],
            tools: Vec::new(),
            thinking: None,
            max_tokens: None,
            temperature: None,
            no_cache: false,
            cache_ttl: None,
            metadata: EventMetadata::empty(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderModelResponse {
    pub status: ProviderModelStatus,
    pub stream_events: Vec<ProviderStreamEvent>,
    pub content: Vec<Content>,
    pub usage: Option<Usage>,
    pub stop_reason: Option<StopReason>,
    pub failure: Option<ProviderModelFailure>,
    pub receipt: ExtensionReceipt,
}

impl ProviderModelResponse {
    #[must_use]
    pub fn completed(
        stream_events: Vec<ProviderStreamEvent>,
        content: Vec<Content>,
        usage: Option<Usage>,
        stop_reason: Option<StopReason>,
        receipt: ExtensionReceipt,
    ) -> Self {
        Self {
            status: ProviderModelStatus::Completed,
            stream_events,
            content,
            usage,
            stop_reason,
            failure: None,
            receipt,
        }
    }

    #[must_use]
    pub fn failure(status: ProviderModelStatus, failure: ProviderModelFailure, receipt: ExtensionReceipt) -> Self {
        assert!(matches!(status, ProviderModelStatus::RetryableFailure | ProviderModelStatus::TerminalFailure));
        Self {
            status,
            stream_events: Vec::new(),
            content: Vec::new(),
            usage: None,
            stop_reason: None,
            failure: Some(failure),
            receipt,
        }
    }

    #[must_use]
    pub fn cancelled(reason: impl Into<String>, receipt: ExtensionReceipt) -> Self {
        Self {
            status: ProviderModelStatus::Cancelled,
            stream_events: Vec::new(),
            content: Vec::new(),
            usage: None,
            stop_reason: None,
            failure: Some(ProviderModelFailure::terminal(reason, None)),
            receipt,
        }
    }
}

pub(crate) fn extension_kind_label(kind: ExtensionRuntimeKind) -> &'static str {
    match kind {
        ExtensionRuntimeKind::Plugin => "plugin",
        ExtensionRuntimeKind::Mcp => "mcp",
        ExtensionRuntimeKind::Gateway => "gateway",
    }
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRecord {
    pub session_id: SessionId,
    pub created_at: DateTime<Utc>,
    pub last_prompt: Option<PromptId>,
    pub prompts: Vec<PromptReplayEntry>,
    #[serde(default)]
    pub ledger_entries: Vec<SessionLedgerEntry>,
}

impl SessionRecord {
    #[must_use]
    pub fn new(session_id: SessionId) -> Self {
        Self {
            session_id,
            created_at: Utc::now(),
            last_prompt: None,
            prompts: Vec::new(),
            ledger_entries: Vec::new(),
        }
    }

    #[must_use]
    pub fn replay_context(&self) -> Vec<PromptReplayEntry> {
        self.prompts.clone()
    }

    pub fn replay(&self) -> Result<crate::SessionLedgerReplay, RuntimeError> {
        crate::replay_ledger_entries(&self.ledger_entries)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptReplayEntry {
    pub prompt_id: PromptId,
    pub user_prompt: String,
    pub assembled_prompt: AssembledPrompt,
    pub completed_at: DateTime<Utc>,
}

pub struct DisabledSessionStore;

impl SessionStore for DisabledSessionStore {
    fn capability(&self) -> &'static str {
        "unsupported"
    }

    fn save(&self, record: SessionRecord) -> Result<(), RuntimeError> {
        let _ = record;
        Err(RuntimeError::SessionUnsupported("session store disabled".to_string()))
    }

    fn load(&self, session_id: &SessionId) -> Result<Option<SessionRecord>, RuntimeError> {
        let _ = session_id;
        Err(RuntimeError::SessionUnsupported("session store disabled".to_string()))
    }
}

pub struct DisabledExtensionService;

impl ProviderRouterService for DisabledExtensionService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn complete(&self, request: ProviderModelRequest) -> Result<ProviderModelResponse, RuntimeError> {
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
