//! Plain LLM contract types shared by message, router, provider, and engine crates.
//!
//! This module intentionally contains only serde-friendly data contracts. It must
//! not depend on provider implementations, router runtime services, async runtimes,
//! databases, network clients, daemon protocols, or UI crates.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::time::Duration;

use clankers_artifacts::ArtifactHash;
use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::content::Content;

/// Tool definition for function calling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// Metadata about an available tool for inventory/projection surfaces.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    /// Source of the tool: "built-in" or plugin name.
    #[serde(default)]
    pub source: String,
}

/// Safe reason a tool was omitted from a published tool catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCatalogOmission {
    pub name: String,
    pub reason: String,
}

impl ToolCatalogOmission {
    #[must_use]
    pub fn new(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self {
            name: sanitize_short_public_value(name.into()),
            reason: sanitize_short_public_value(reason.into()),
        }
    }
}

/// Minimal serialized message used for seeding and replaying session history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SerializedMessage {
    pub role: String,
    pub content: String,
    pub model: Option<String>,
    pub timestamp: Option<String>,
}

/// Identifies a daemon session by transport and sender.
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub enum SessionKey {
    /// iroh peer identified by public key.
    Iroh(String),
    /// Matrix user in a room.
    Matrix { user_id: String, room_id: String },
}

impl std::fmt::Display for SessionKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iroh(id) => write!(f, "iroh:{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => write!(f, "matrix:{}@{}", user_id, room_id),
        }
    }
}

impl SessionKey {
    /// Deterministic directory name for this session's working files.
    pub fn dir_name(&self) -> String {
        match self {
            Self::Iroh(id) => format!("daemon_iroh_{}", &id[..12.min(id.len())]),
            Self::Matrix { user_id, room_id } => {
                let user = user_id.replace(':', "_").replace('@', "");
                let room = room_id.replace(':', "_").replace('!', "");
                format!("daemon_matrix_{}_{}", user, room)
            }
        }
    }

    /// Extract the Matrix room_id if this is a Matrix session.
    pub fn matrix_room_id(&self) -> Option<&str> {
        match self {
            Self::Matrix { room_id, .. } => Some(room_id),
            _ => None,
        }
    }
}

/// Summary of an active daemon session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    pub session_id: String,
    pub model: String,
    pub turn_count: usize,
    pub last_active: String,
    pub client_count: usize,
    pub socket_path: String,
    /// Lifecycle state: "active", "suspended", or "recovering".
    #[serde(default = "default_session_state")]
    pub state: String,
}

fn default_session_state() -> String {
    "active".to_string()
}

/// Daemon runtime status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DaemonStatus {
    pub uptime_secs: f64,
    pub session_count: usize,
    pub total_clients: usize,
    pub pid: u32,
}

/// Named thinking budget levels shared by provider, controller, and display edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThinkingLevel {
    /// Thinking disabled.
    Off,
    /// Quick reasoning (~5k tokens).
    Low,
    /// Moderate reasoning (~10k tokens).
    Medium,
    /// Deep reasoning (~32k tokens).
    High,
    /// Maximum reasoning (~128k tokens).
    Max,
}

impl ThinkingLevel {
    /// Token budget for this level (None for Off).
    pub const fn budget_tokens(self) -> Option<u32> {
        match self {
            Self::Off => None,
            Self::Low => Some(5_000),
            Self::Medium => Some(10_000),
            Self::High => Some(32_000),
            Self::Max => Some(128_000),
        }
    }

    /// Whether thinking is enabled at this level.
    pub const fn is_enabled(self) -> bool {
        !matches!(self, Self::Off)
    }

    /// Cycle to the next level.
    pub const fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Max,
            Self::Max => Self::Off,
        }
    }

    /// Display name.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Max => "max",
        }
    }

    /// Parse from a string level name.
    pub fn from_str_or_budget(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "off" | "none" | "disable" | "disabled" => Some(Self::Off),
            "low" | "lo" | "l" => Some(Self::Low),
            "medium" | "med" | "m" => Some(Self::Medium),
            "high" | "hi" | "h" => Some(Self::High),
            "xhigh" | "x-high" | "extra-high" | "max" | "maximum" | "full" | "default" => Some(Self::Max),
            _ => None,
        }
    }

    /// Find the closest level for a raw token budget.
    pub const fn from_budget(tokens: u32) -> Self {
        if tokens == 0 {
            Self::Off
        } else if tokens <= 5_000 {
            Self::Low
        } else if tokens <= 10_000 {
            Self::Medium
        } else if tokens <= 32_000 {
            Self::High
        } else {
            Self::Max
        }
    }

    /// All levels in order.
    pub const fn all() -> &'static [Self] {
        &[Self::Off, Self::Low, Self::Medium, Self::High, Self::Max]
    }
}

/// Configuration for extended thinking mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether extended thinking is enabled.
    pub enabled: bool,
    /// Maximum tokens for thinking.
    pub budget_tokens: Option<usize>,
}

const RUNTIME_RETRY_DELAY_MS_MAX: u64 = 365 * 24 * 60 * 60 * 1000;

/// Role for provider messages exchanged with host model adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMessageRole {
    User,
    Assistant,
    Tool,
    System,
}

/// Provider message exchanged with host model adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMessage {
    pub role: ProviderMessageRole,
    pub content: Vec<Content>,
    pub id: Option<String>,
    pub model: Option<String>,
    pub call_id: Option<String>,
    pub tool_name: Option<String>,
    pub is_error: bool,
}

impl ProviderMessage {
    #[must_use]
    pub fn user_text(prompt: impl Into<String>) -> Self {
        Self {
            role: ProviderMessageRole::User,
            content: vec![Content::Text { text: prompt.into() }],
            id: None,
            model: None,
            call_id: None,
            tool_name: None,
            is_error: false,
        }
    }

    #[must_use]
    pub fn assistant(content: Vec<Content>, model: Option<String>) -> Self {
        Self {
            role: ProviderMessageRole::Assistant,
            content,
            id: None,
            model,
            call_id: None,
            tool_name: None,
            is_error: false,
        }
    }

    #[must_use]
    pub fn tool_result(
        call_id: impl Into<String>,
        tool_name: impl Into<String>,
        content: Vec<Content>,
        is_error: bool,
    ) -> Self {
        Self {
            role: ProviderMessageRole::Tool,
            content,
            id: None,
            model: None,
            call_id: Some(call_id.into()),
            tool_name: Some(tool_name.into()),
            is_error,
        }
    }
}

/// Provider stream event exchanged with host model adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ProviderStreamEvent {
    MessageStart {
        model: String,
        role: String,
    },
    ContentBlockStart {
        index: usize,
        content: Content,
    },
    TextDelta {
        index: usize,
        text: String,
    },
    ThinkingDelta {
        index: usize,
        thinking: String,
    },
    ToolInputJsonDelta {
        index: usize,
        partial_json: String,
    },
    SignatureDelta {
        index: usize,
        signature: String,
    },
    ContentBlockStop {
        index: usize,
    },
    Usage {
        stop_reason: Option<crate::content::StopReason>,
        usage: Usage,
    },
    MessageStop,
    Error {
        message: String,
    },
}

/// Provider model call status exchanged with host model adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderModelStatus {
    Completed,
    RetryableFailure,
    TerminalFailure,
    Cancelled,
}

/// Provider model failure details exchanged with host model adapters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderModelFailure {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
}

impl ProviderModelFailure {
    #[must_use]
    pub fn retryable(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: sanitize_short_public_value(message.into()),
            status,
            retryable: true,
        }
    }

    #[must_use]
    pub fn terminal(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: sanitize_short_public_value(message.into()),
            status,
            retryable: false,
        }
    }
}

fn sanitize_short_public_value(value: String) -> String {
    let lower = value.to_ascii_lowercase();
    let contains_secret = [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker));
    if contains_secret {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

/// Host confirmation action requested before side effects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationAction {
    RunCommand,
    MutateWorkspace,
    UseNetwork,
    Custom(String),
}

/// Host confirmation decision.
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
            reason: sanitize_short_public_value(reason.into()),
        }
    }

    #[must_use]
    pub fn deny(reason: impl Into<String>) -> Self {
        Self {
            approved: false,
            reason: sanitize_short_public_value(reason.into()),
        }
    }
}

/// Safe runtime error class for event and receipt projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorClass {
    InvalidInput,
    Session,
    Policy,
    Tooling,
    Storage,
    Confirmation,
    Extension,
    Boundary,
    Model,
}

/// Operation requested from a host-owned auth store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStoreOperation {
    Lookup,
    RefreshPersist,
    PendingLoginVerifier,
}

/// Request to resolve named skills from a host skill service.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillResolutionRequest {
    pub requested: Vec<String>,
}

/// Resolved skill snippet returned by a host skill service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedSkillSnippet {
    pub name: String,
    pub description: String,
    pub content: String,
    pub source: String,
}

/// Request to collect host prompt sources before prompt assembly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PromptSourceRequest {
    pub user_prompt: String,
    pub policy: PromptAssemblyPolicy,
}

/// Prompt assembly feature policy supplied by the host.
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

/// Prompt source material returned by the host.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PromptSources {
    pub system_prompt: Option<String>,
    pub host_context: Vec<HostContext>,
    #[serde(default)]
    pub filesystem_context: Vec<HostContext>,
    pub filesystem_context_requested: bool,
    pub context_references: Vec<ContextReferenceRequest>,
    #[serde(default)]
    pub skill_snippets: Vec<SkillSnippet>,
}

/// Host-supplied context block for prompt assembly.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostContext {
    pub label: String,
    pub content: String,
}

/// Skill snippet included in an assembled prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillSnippet {
    pub name: String,
    pub content: String,
    pub source: String,
}

/// Prompt model request metadata supplied by a host runtime adapter.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ModelRequestMetadata {
    pub request_id: String,
    pub message_count: usize,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub tool_names: Vec<String>,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
}

/// Model adapter failure returned by prompt execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelFailure {
    pub message: String,
    pub status: Option<u16>,
    pub retryable: bool,
}

impl ModelFailure {
    #[must_use]
    pub fn retryable(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: message.into(),
            status,
            retryable: true,
        }
    }

    #[must_use]
    pub fn terminal(message: impl Into<String>, status: Option<u16>) -> Self {
        Self {
            message: message.into(),
            status,
            retryable: false,
        }
    }
}

/// Prompt assembled from host sources and user input.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AssembledPrompt {
    pub user_prompt: String,
    pub sections: Vec<PromptSection>,
    pub provenance: Vec<PromptProvenance>,
    pub context_references_enabled: bool,
    pub unsupported_context_references: Vec<UnsupportedContextReference>,
}

/// Named prompt section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptSection {
    pub label: String,
    pub content: String,
}

/// Provenance entry for an assembled prompt section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PromptProvenance {
    pub label: String,
    pub source: PromptSourceKind,
    pub safe_summary: String,
}

/// Prompt source kind for provenance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromptSourceKind {
    Host,
    Filesystem,
    Skill,
    Generated,
}

/// Request to resolve a context reference.
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

/// Kind of context reference requested by prompt assembly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextReferenceKind {
    File,
    Directory,
    Url,
    Custom,
}

/// Context reference rejected by host policy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnsupportedContextReference {
    pub label: String,
    pub kind: ContextReferenceKind,
    pub reason: String,
}

/// Kind of host extension runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionRuntimeKind {
    Plugin,
    Mcp,
    Gateway,
}

/// Request for host-owned auth store access.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthStoreAccessRequest {
    pub provider: String,
    pub account_label: Option<String>,
    pub operation: AuthStoreOperation,
}

/// Request for host credential-pool policy selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialPoolRequest {
    pub provider: String,
    pub strategy: String,
    pub account_label: Option<String>,
}

/// Request for host extension runtime execution.
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

/// Effectful capability class requested by runtime/tool code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectAbilityClass {
    Filesystem,
    Shell,
    Network,
    Secret,
    Browser,
    Scheduler,
    Provider,
    Plugin,
    Tool,
    Delivery,
}

/// Handler outcome kind for a typed effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectResultStatus {
    Allowed,
    Denied,
    Simulated,
    Replayed,
    Unavailable,
}

/// Stable correlation identifier carried through requests, results, and receipts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EffectCorrelationId(String);

impl EffectCorrelationId {
    /// Construct from an opaque ID string supplied by a runtime host.
    #[must_use]
    pub fn from_string(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Construct from a known deterministic ID for tests/replay.
    #[must_use]
    pub fn from_static(id: &'static str) -> Self {
        Self(id.to_owned())
    }

    /// Borrow the ID as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Policy-relevant effect request envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRequest {
    /// Requested ability class.
    pub class: EffectAbilityClass,
    /// Correlation ID for matching handler receipts/replay results.
    pub correlation_id: EffectCorrelationId,
    /// Content hash of the input schema or tool descriptor that shaped the request.
    pub input_schema_hash: Option<ArtifactHash>,
    /// Declared artifacts required to safely understand/execute the request.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub declared_artifact_dependencies: Vec<ArtifactHash>,
    /// Redaction class for request/result receipt material.
    pub redaction_class: RedactionClass,
    /// Safe source metadata for review logs; values are sanitized and secret markers rejected.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub safe_source_metadata: BTreeMap<String, String>,
}

impl EffectRequest {
    /// Create a request envelope with an explicit class, correlation ID, and redaction policy.
    #[must_use]
    pub fn new(
        class: EffectAbilityClass,
        correlation_id: EffectCorrelationId,
        redaction_class: RedactionClass,
    ) -> Self {
        Self {
            class,
            correlation_id,
            input_schema_hash: None,
            declared_artifact_dependencies: Vec::new(),
            redaction_class,
            safe_source_metadata: BTreeMap::new(),
        }
    }

    /// Attach an input schema/tool descriptor hash.
    #[must_use]
    pub fn with_input_schema_hash(mut self, hash: ArtifactHash) -> Self {
        self.input_schema_hash = Some(hash);
        self
    }

    /// Attach artifact dependencies in deterministic order.
    #[must_use]
    pub fn with_artifact_dependencies<I>(mut self, dependencies: I) -> Self
    where I: IntoIterator<Item = ArtifactHash> {
        self.declared_artifact_dependencies = dependencies.into_iter().collect();
        self.declared_artifact_dependencies.sort_by_key(|hash| hash.hex());
        self.declared_artifact_dependencies.dedup();
        self
    }

    /// Add safe source metadata. Secret-looking values are replaced with a redaction marker.
    #[must_use]
    pub fn with_safe_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = sanitize_effect_metadata_value(key.into());
        let raw_value = value.into();
        let value = if contains_effect_secret_marker(&raw_value) {
            "[redacted-secret-marker]".to_owned()
        } else {
            sanitize_effect_metadata_value(raw_value)
        };
        self.safe_source_metadata.insert(key, value);
        self
    }
}

/// Minimal request reference copied into results/receipts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRequestRef {
    /// Requested ability class.
    pub class: EffectAbilityClass,
    /// Correlation ID used to match request and result.
    pub correlation_id: EffectCorrelationId,
    /// Redaction class applied to result receipt data.
    pub redaction_class: RedactionClass,
}

impl From<&EffectRequest> for EffectRequestRef {
    fn from(request: &EffectRequest) -> Self {
        Self {
            class: request.class,
            correlation_id: request.correlation_id.clone(),
            redaction_class: request.redaction_class,
        }
    }
}

/// Redacted effect result/receipt envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectResult {
    /// Request reference this result answers.
    pub request: EffectRequestRef,
    /// Handler outcome status.
    pub status: EffectResultStatus,
    /// Optional content-addressed result artifact.
    pub output_artifact: Option<ArtifactHash>,
    /// Safe, sanitized summary suitable for logs/review receipts.
    pub safe_summary: String,
    /// Optional safe UCAN authorization metadata; never contains compact tokens or secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ucan_authorization: Option<UcanAuthorizationMetadata>,
}

impl EffectResult {
    /// Build a redacted result envelope for a request.
    #[must_use]
    pub fn new(request: &EffectRequest, status: EffectResultStatus, safe_summary: impl Into<String>) -> Self {
        let raw_summary = safe_summary.into();
        let safe_summary = if contains_effect_secret_marker(&raw_summary) {
            "[redacted-secret-marker]".to_owned()
        } else {
            sanitize_effect_metadata_value(raw_summary)
        };
        Self {
            request: EffectRequestRef::from(request),
            status,
            output_artifact: None,
            safe_summary,
            ucan_authorization: None,
        }
    }

    /// Attach a result artifact hash.
    #[must_use]
    pub fn with_output_artifact(mut self, hash: ArtifactHash) -> Self {
        self.output_artifact = Some(hash);
        self
    }

    /// Attach redacted UCAN authorization metadata to the receipt.
    #[must_use]
    pub fn with_ucan_authorization(mut self, metadata: UcanAuthorizationMetadata) -> Self {
        self.ucan_authorization = Some(metadata);
        self
    }
}

fn sanitize_effect_metadata_value(value: String) -> String {
    if contains_effect_secret_marker(&value) {
        "[REDACTED]".to_string()
    } else {
        value.chars().take(160).collect()
    }
}

fn contains_effect_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

/// Safe UCAN authorization metadata for effect receipts and sync envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UcanAuthorizationMetadata {
    /// Stable ability string checked by the UCAN adapter.
    pub ability: String,
    /// Stable resource URI checked by the UCAN adapter.
    pub resource_uri: String,
    /// Allowed, denied, replayed, revoked, or unavailable authorization status.
    pub status: String,
    /// Safe issuer DID, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issuer: Option<String>,
    /// Safe audience DID, when available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audience: Option<String>,
    /// Safe proof-chain or grant references, never raw compact token strings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub proof_references: Vec<String>,
    /// Safe caveat identifiers/classes evaluated for this decision.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveat_ids: Vec<String>,
    /// Replay admission status, when replay checking was involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replay_status: Option<String>,
    /// Revocation status, when revocation checking was involved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revocation_status: Option<String>,
    /// Redacted denial class for denied authorization receipts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub denial_class: Option<String>,
}

impl UcanAuthorizationMetadata {
    /// Build safe receipt metadata. Inputs are sanitized and secret-looking values are redacted.
    #[must_use]
    pub fn new(ability: impl Into<String>, resource_uri: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            ability: sanitize_authorization_metadata_value(ability.into()),
            resource_uri: sanitize_authorization_metadata_value(resource_uri.into()),
            status: sanitize_authorization_metadata_value(status.into()),
            issuer: None,
            audience: None,
            proof_references: Vec::new(),
            caveat_ids: Vec::new(),
            replay_status: None,
            revocation_status: None,
            denial_class: None,
        }
    }

    #[must_use]
    pub fn with_issuer(mut self, issuer: impl Into<String>) -> Self {
        self.issuer = Some(sanitize_authorization_metadata_value(issuer.into()));
        self
    }

    #[must_use]
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(sanitize_authorization_metadata_value(audience.into()));
        self
    }

    #[must_use]
    pub fn with_proof_references<I, S>(mut self, references: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.proof_references = sorted_sanitized_authorization_values(references);
        self
    }

    #[must_use]
    pub fn with_caveat_ids<I, S>(mut self, caveat_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.caveat_ids = sorted_sanitized_authorization_values(caveat_ids);
        self
    }

    #[must_use]
    pub fn with_replay_status(mut self, replay_status: impl Into<String>) -> Self {
        self.replay_status = Some(sanitize_authorization_metadata_value(replay_status.into()));
        self
    }

    #[must_use]
    pub fn with_revocation_status(mut self, revocation_status: impl Into<String>) -> Self {
        self.revocation_status = Some(sanitize_authorization_metadata_value(revocation_status.into()));
        self
    }

    #[must_use]
    pub fn with_denial_class(mut self, denial_class: impl Into<String>) -> Self {
        self.denial_class = Some(sanitize_authorization_metadata_value(denial_class.into()));
        self
    }
}

fn sorted_sanitized_authorization_values<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut sanitized = values
        .into_iter()
        .map(|value| sanitize_authorization_metadata_value(value.into()))
        .collect::<Vec<_>>();
    sanitized.sort();
    sanitized.dedup();
    sanitized
}

fn sanitize_authorization_metadata_value(value: String) -> String {
    if contains_authorization_secret_marker(&value) || looks_like_compact_token(&value) {
        "[redacted-secret-marker]".to_owned()
    } else {
        value.chars().take(160).collect()
    }
}

fn contains_authorization_secret_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "token",
        "secret",
        "password",
        "api_key",
        "authorization",
        "bearer",
        "cookie",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn looks_like_compact_token(value: &str) -> bool {
    value.matches('.').count() == 2 && value.starts_with("ey") && value.len() > 80
}

/// Safe content-addressed artifact kind declared by remote/subagent execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteExecutionArtifactKind {
    Prompt,
    Skill,
    ToolSchema,
    Manifest,
    Policy,
}

/// Fail-closed remote dependency sync failure kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteDependencyFailureKind {
    MissingSafeArtifact,
    UnsupportedVersion,
    HashMismatch,
    SecretDependencyDenied,
}

/// Remote/subagent execution target shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteExecutionTarget {
    Subagent,
    RemoteDaemon,
}

/// One declared remote/subagent dependency bound to a content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteExecutionDependency {
    /// Safe artifact kind.
    pub kind: RemoteExecutionArtifactKind,
    /// Content-addressed artifact identity.
    pub hash: ArtifactHash,
}

impl RemoteExecutionDependency {
    /// Build a safe dependency declaration.
    #[must_use]
    pub fn new(kind: RemoteExecutionArtifactKind, hash: ArtifactHash) -> Self {
        Self { kind, hash }
    }
}

/// Typed remote/subagent execution request preflight declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteExecutionRequest {
    /// Stable correlation ID for remote preflight, sync, and execution receipts.
    pub correlation_id: EffectCorrelationId,
    /// Whether this is local subagent execution or a remote daemon peer.
    pub target: RemoteExecutionTarget,
    /// Required safe artifacts, normalized by kind/hash.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub required_artifacts: Vec<RemoteExecutionDependency>,
}

impl RemoteExecutionRequest {
    /// Create a remote/subagent dependency declaration.
    #[must_use]
    pub fn new(target: RemoteExecutionTarget, correlation_id: EffectCorrelationId) -> Self {
        Self {
            correlation_id,
            target,
            required_artifacts: Vec::new(),
        }
    }

    /// Attach safe artifact dependencies in deterministic order.
    #[must_use]
    pub fn with_required_artifacts<I>(mut self, artifacts: I) -> Self
    where I: IntoIterator<Item = RemoteExecutionDependency> {
        self.required_artifacts = artifacts.into_iter().collect();
        self.required_artifacts
            .sort_by(|left, right| left.kind.cmp(&right.kind).then_with(|| left.hash.hex().cmp(&right.hash.hex())));
        self.required_artifacts.dedup();
        self
    }

    /// Return a plain hash set projection for effect request dependencies.
    #[must_use]
    pub fn required_hashes(&self) -> Vec<ArtifactHash> {
        let mut hashes = self.required_artifacts.iter().map(|dependency| dependency.hash).collect::<Vec<_>>();
        hashes.sort_by_key(|hash| hash.hex());
        hashes.dedup();
        hashes
    }
}

/// Supported schema version for safe remote artifact envelopes.
pub const REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION: u32 = 1;

/// Safe artifact envelope advertised or transferred during remote dependency sync.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteArtifactEnvelope {
    /// Requested dependency identity.
    pub dependency: RemoteExecutionDependency,
    /// Envelope schema version understood by this runtime.
    pub schema_version: u32,
    /// Hash recomputed from the canonical envelope body by the receiver.
    pub computed_hash: ArtifactHash,
    /// Redaction class of the envelope body. Secret envelopes are never syncable.
    pub redaction_class: RedactionClass,
    /// Optional safe UCAN authorization metadata; never contains compact tokens or secrets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ucan_authorization: Option<UcanAuthorizationMetadata>,
}

impl RemoteArtifactEnvelope {
    /// Build an envelope receipt for remote sync preflight.
    #[must_use]
    pub fn new(
        dependency: RemoteExecutionDependency,
        schema_version: u32,
        computed_hash: ArtifactHash,
        redaction_class: RedactionClass,
    ) -> Self {
        Self {
            dependency,
            schema_version,
            computed_hash,
            redaction_class,
            ucan_authorization: None,
        }
    }

    /// Attach redacted UCAN authorization metadata to the envelope.
    #[must_use]
    pub fn with_ucan_authorization(mut self, metadata: UcanAuthorizationMetadata) -> Self {
        self.ucan_authorization = Some(metadata);
        self
    }
}

/// Redacted remote dependency failure receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDependencyFailure {
    /// Dependency involved in the failure.
    pub dependency: RemoteExecutionDependency,
    /// Failure kind.
    pub kind: RemoteDependencyFailureKind,
    /// Safe redacted summary.
    pub safe_summary: String,
}

impl RemoteDependencyFailure {
    /// Build a redacted remote dependency failure receipt.
    #[must_use]
    pub fn new(
        dependency: RemoteExecutionDependency,
        kind: RemoteDependencyFailureKind,
        safe_summary: impl Into<String>,
    ) -> Self {
        Self {
            dependency,
            kind,
            safe_summary: sanitize_short_public_value(safe_summary.into()),
        }
    }
}

/// Fail-closed remote dependency sync preflight report.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemoteDependencySyncReport {
    /// Safe artifacts the peer should request by hash before execution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub missing_safe_artifacts: Vec<RemoteExecutionDependency>,
    /// Failures that abort execution before side effects.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<RemoteDependencyFailure>,
}

impl RemoteDependencySyncReport {
    /// True only when every declared dependency is present, supported, non-secret, and
    /// hash-matched.
    #[must_use]
    pub fn ready(&self) -> bool {
        self.missing_safe_artifacts.is_empty() && self.failures.is_empty()
    }

    /// Convert the preflight outcome into an effect result for fail-closed dispatch.
    #[must_use]
    pub fn to_effect_result(&self, request: &EffectRequest) -> EffectResult {
        if self.ready() {
            EffectResult::new(request, EffectResultStatus::Allowed, "remote dependencies ready")
        } else {
            EffectResult::new(request, EffectResultStatus::Unavailable, "remote dependencies unavailable")
        }
    }
}

/// Dynamic runtime implementation kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeKind {
    SteelScheme,
    Wasm,
}

/// Dynamic runtime action kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionKind {
    HostFunction,
    Tool,
}

/// Dynamic runtime redaction policy for input material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeRedactionClass {
    PublicSummary,
    MetadataOnly,
    SecretBearing,
}

/// Dynamic runtime authorization result status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionStatus {
    Allowed,
    PolicyDenied,
    UcanDenied,
    Disabled,
    InvalidEnvelope,
}

/// Dynamic runtime authorization reason code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DynamicRuntimeActionReason {
    Ready,
    InvalidSchema,
    MissingRequiredField,
    UnsupportedRuntimeProfile,
    UnsupportedAction,
    DisabledAction,
    MissingSessionCapability,
    MissingUcanAbility,
    SecretBearingInput,
    InputTooLarge,
    UnsafeReceiptDestination,
    UnsafeTargetResource,
}

pub const DYNAMIC_RUNTIME_ACTION_SCHEMA: &str = "clankers.dynamic_runtime.action.v1";
pub const DYNAMIC_RUNTIME_RECEIPT_SCHEMA: &str = "clankers.dynamic_runtime.action_receipt.v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeActionEnvelope {
    pub schema: String,
    pub action_id: String,
    pub runtime: DynamicRuntimeKind,
    pub runtime_profile: String,
    pub action_kind: DynamicRuntimeActionKind,
    pub action_name: String,
    pub target_resource: String,
    pub receipt_destination: String,
    pub required_ucan_ability: String,
    pub required_session_capabilities: Vec<String>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub redaction: DynamicRuntimeRedactionClass,
}

impl DynamicRuntimeActionEnvelope {
    #[must_use]
    pub fn stable_action_key(&self) -> String {
        format!("{}:{}", dynamic_runtime_action_kind_tag(self.action_kind), self.action_name)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeAuthorizationContext {
    pub allowed_runtime_profiles: BTreeSet<String>,
    pub allowed_actions: BTreeSet<String>,
    pub granted_ucan_abilities: BTreeSet<String>,
    pub session_capabilities: BTreeSet<String>,
    pub disabled_actions: BTreeSet<String>,
    pub max_input_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicRuntimeActionReceipt {
    pub schema: String,
    pub action_id: String,
    pub runtime: DynamicRuntimeKind,
    pub runtime_profile: String,
    pub action_kind: DynamicRuntimeActionKind,
    pub action_name: String,
    pub target_resource: String,
    pub receipt_destination: String,
    pub status: DynamicRuntimeActionStatus,
    pub reason: DynamicRuntimeActionReason,
    pub safe_summary: String,
    pub required_ucan_ability: String,
    pub required_session_capabilities: Vec<String>,
    pub input_hash: ArtifactHash,
    pub input_bytes: u64,
    pub writes_performed: bool,
    pub receipt_hash: ArtifactHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationProfile {
    pub runtime_profile: String,
    pub allowed_host_functions: BTreeSet<String>,
    pub required_session_capabilities: Vec<String>,
    pub default_ucan_ability: String,
    pub receipt_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationRequest {
    pub script_id: String,
    pub route_hint: String,
    pub target_resource: String,
    pub requested_host_function: String,
    pub input_summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FakeSteelOrchestrationReceipt {
    pub selected_action: DynamicRuntimeActionEnvelope,
    pub authorization_receipt: DynamicRuntimeActionReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionProfile {
    pub runtime_profile: String,
    pub allowed_imports: BTreeSet<String>,
    pub required_session_capabilities: Vec<String>,
    pub required_ucan_ability: String,
    pub max_memory_pages: u32,
    pub max_fuel: u64,
    pub max_time_ms: u64,
    pub input_schema: String,
    pub output_schema: String,
    pub receipt_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionRequest {
    pub tool_name: String,
    pub target_resource: String,
    pub required_imports: Vec<String>,
    pub input_summary: String,
    pub requested_memory_pages: u32,
    pub requested_fuel: u64,
    pub requested_time_ms: u64,
    pub input_schema: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WasmToolExecutionReceipt {
    pub status: WasmToolExecutionStatus,
    pub authorization_receipt: DynamicRuntimeActionReceipt,
    pub used_imports: Vec<String>,
    pub memory_pages: u32,
    pub fuel: u64,
    pub time_ms: u64,
    pub output_hash: Option<ArtifactHash>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossLayerFixtureReceipt {
    pub nickel_profile_validated: bool,
    pub steel_route_receipt: FakeSteelOrchestrationReceipt,
    pub wasm_execution_receipt: WasmToolExecutionReceipt,
}

fn dynamic_runtime_action_kind_tag(kind: DynamicRuntimeActionKind) -> &'static str {
    match kind {
        DynamicRuntimeActionKind::HostFunction => "host_function",
        DynamicRuntimeActionKind::Tool => "tool",
    }
}

/// Ambient host access kind requested by dynamic runtime code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelAmbientAccessKind {
    Filesystem,
    Shell,
    Git,
    Network,
    Provider,
    Credential,
    Daemon,
    Tui,
    NativeTool,
}

impl SteelAmbientAccessKind {
    #[must_use]
    pub fn all() -> [Self; 9] {
        [
            Self::Filesystem,
            Self::Shell,
            Self::Git,
            Self::Network,
            Self::Provider,
            Self::Credential,
            Self::Daemon,
            Self::Tui,
            Self::NativeTool,
        ]
    }

    #[must_use]
    pub const fn host_function_name(self) -> &'static str {
        match self {
            Self::Filesystem => "steel.ambient.fs",
            Self::Shell => "steel.ambient.shell",
            Self::Git => "steel.ambient.git",
            Self::Network => "steel.ambient.network",
            Self::Provider => "steel.ambient.provider",
            Self::Credential => "steel.ambient.credential",
            Self::Daemon => "steel.ambient.daemon",
            Self::Tui => "steel.ambient.tui",
            Self::NativeTool => "steel.ambient.native_tool",
        }
    }

    #[must_use]
    pub const fn target_resource(self) -> &'static str {
        match self {
            Self::Filesystem => "fs:ambient",
            Self::Shell => "process:shell",
            Self::Git => "git:ambient",
            Self::Network => "network:ambient",
            Self::Provider => "provider:ambient",
            Self::Credential => "credential:ambient",
            Self::Daemon => "daemon:ambient",
            Self::Tui => "tui:ambient",
            Self::NativeTool => "native-tool:ambient",
        }
    }

    #[must_use]
    pub const fn route_hint(self) -> &'static str {
        match self {
            Self::Filesystem => "raw filesystem",
            Self::Shell => "shell command",
            Self::Git => "git operation",
            Self::Network => "network request",
            Self::Provider => "provider call",
            Self::Credential => "credential read",
            Self::Daemon => "daemon access",
            Self::Tui => "tui mutation",
            Self::NativeTool => "native tool",
        }
    }
}

/// Wasm tool execution result status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WasmToolExecutionStatus {
    Completed,
    Blocked,
}

const DEFAULT_STEEL_PROFILE_NAME: &str = "default-deny";
const DEFAULT_STEEL_MAX_SOURCE_BYTES: u64 = 4096;
const DEFAULT_STEEL_MAX_OUTPUT_BYTES: u64 = 1024;
const DEFAULT_STEEL_MAX_HOST_CALLS: u64 = 4;
const DEFAULT_STEEL_MAX_STEPS: u64 = 256;

/// Steel runtime profile limits and authority flags.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeProfile {
    pub name: String,
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
    pub max_steps: u64,
    pub ambient_authority: bool,
    pub agent_tool_enabled: bool,
}

impl SteelRuntimeProfile {
    #[must_use]
    pub fn default_deny() -> Self {
        Self {
            name: DEFAULT_STEEL_PROFILE_NAME.to_string(),
            max_source_bytes: DEFAULT_STEEL_MAX_SOURCE_BYTES,
            max_output_bytes: DEFAULT_STEEL_MAX_OUTPUT_BYTES,
            max_host_calls: DEFAULT_STEEL_MAX_HOST_CALLS,
            max_steps: DEFAULT_STEEL_MAX_STEPS,
            ambient_authority: false,
            agent_tool_enabled: false,
        }
    }
}

/// Steel host function made available to a constrained runtime profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelHostFunctionRegistration {
    pub name: String,
    pub required_capability: String,
    pub output: String,
}

/// Steel runtime evaluation request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeRequest {
    pub profile: SteelRuntimeProfile,
    pub source: String,
    pub session_capabilities: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub host_functions: Vec<SteelHostFunctionRegistration>,
    pub receipt_destination: String,
}

impl SteelRuntimeRequest {
    #[must_use]
    pub fn pure(source: impl Into<String>) -> Self {
        Self {
            profile: SteelRuntimeProfile::default_deny(),
            source: source.into(),
            session_capabilities: Vec::new(),
            disabled_tools: Vec::new(),
            host_functions: Vec::new(),
            receipt_destination: "stdout".to_string(),
        }
    }
}

/// Steel runtime availability and sandbox status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelRuntimeStatus {
    pub schema: String,
    pub available: bool,
    pub implementation: String,
    pub profile: SteelRuntimeProfile,
    pub agent_tool_enabled: bool,
    pub ambient_authority: bool,
    pub sandbox_claim: String,
}

/// Steel runtime evaluation status code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelRuntimeStatusCode {
    Succeeded,
    Denied,
    ResourceLimited,
    EvaluationFailed,
}

/// Steel runtime evaluation reason code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelRuntimeReasonCode {
    Ok,
    SourceTooLarge,
    OutputTooLarge,
    ExecutionBudgetExceeded,
    HostCallBudgetExceeded,
    UnknownHostFunction,
    DisabledHostFunction,
    MissingHostCapability,
    AmbientAuthorityDenied,
    UnsupportedExpression,
}

/// Steel host-call authorization outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelHostCallOutcome {
    Approved,
    Denied,
}

/// Safe Steel host-call receipt entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelHostCallReceipt {
    pub name: String,
    pub outcome: SteelHostCallOutcome,
    pub safe_message: String,
}

/// Runtime selected to execute a Steel-mediated tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolExecutorKind {
    RustBuiltin,
    WasmPlugin,
    StdioPlugin,
    Subagent,
}

impl SteelToolExecutorKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RustBuiltin => "rust_builtin",
            Self::WasmPlugin => "wasm_plugin",
            Self::StdioPlugin => "stdio_plugin",
            Self::Subagent => "subagent",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "rust_builtin" => Some(Self::RustBuiltin),
            "wasm_plugin" => Some(Self::WasmPlugin),
            "stdio_plugin" => Some(Self::StdioPlugin),
            "subagent" => Some(Self::Subagent),
            _ => None,
        }
    }
}

/// Steel tool substrate rollout stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateRolloutStage {
    Disabled,
    Comparison,
    Default,
    Block,
}

/// Steel tool substrate fallback behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateFallbackMode {
    RustNative,
    Block,
}

/// Steel tool substrate receipt status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelToolSubstrateStatus {
    Authorized,
    FallbackUsed,
    Blocked,
    Denied,
    Failed,
}

/// Steel tool substrate issue code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelToolSubstrateIssue {
    Ok,
    Disabled,
    ComparisonMode,
    ExecutorKindDenied,
    ToolDisabled,
    InputTooLarge,
    MissingSessionCapability,
    MissingUcanAbility,
    RuntimeFailed,
    MalformedPlan,
}

/// Steel turn orchestration rollout stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationRolloutStage {
    Disabled,
    Comparison,
    Default,
}

/// Steel turn orchestration fallback behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationFallbackMode {
    RustNative,
    Block,
}

/// Planner implementation selected for a turn orchestration receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationPlannerKind {
    SteelScheme,
    RustNative,
}

/// Steel turn orchestration plan authorization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OrchestrationPlanStatus {
    Authorized,
    Denied,
    FallbackUsed,
    Blocked,
}

/// Steel turn orchestration issue code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OrchestrationIssueCode {
    Ok,
    SteelDisabled,
    UnsupportedSeam,
    ScriptEvaluationFailed,
    MalformedPlan,
    FallbackDisabled,
    NoCandidateActions,
    UnauthorizedAction,
    BasaltRequestInvalid,
    BasaltReceiptInvalid,
    UcanAuthorityDenied,
}

/// Rust-native fallback status for Steel turn orchestration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RustNativeFallbackStatus {
    NotNeeded,
    Used,
    Disabled,
    Unavailable,
}

/// UCAN/Basalt authority decision status for Steel turn planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelTurnPlanningAuthorityStatus {
    Allowed,
    Denied,
}

/// UCAN/Basalt authority decision reason for Steel turn planning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelTurnPlanningAuthorityReason {
    Allowed,
    MissingGrant,
    ExpiredGrant,
    RevokedGrant,
    WrongAudience,
    WrongResource,
    WrongAbility,
    UnknownCaveat,
    OverbroadGrant,
    BasaltDenied,
    BasaltError,
}

/// Steel-mediated turn execution authorization status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelTurnExecutionStatus {
    Authorized,
    Denied,
}

/// Repo-local Steel evolution fallback behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelRepoEvolutionFallbackMode {
    RustNative,
    Block,
}

/// Repo-local Steel evolution activation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelRepoEvolutionActivationStatus {
    Inactive,
    Active,
    Denied,
}

/// Repo-local Steel evolution activation reason code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelRepoEvolutionActivationReason {
    AbsentPack,
    Active,
    InvalidProfileJson,
    InvalidSchema,
    InvalidAbiVersion,
    MissingNickelProfile,
    ReadNickelContract,
    InvalidNickelContract,
    MissingScript,
    PathEscape,
    ScriptHashMismatch,
    ScriptTooLarge,
    EmptyScripts,
    EmptyHostCalls,
    UnknownHostCall,
    MissingHostContract,
    InvalidHigherOrderContract,
    ReceiptRootEscape,
    BudgetTooSmall,
}

/// Repo-local Steel evolution plan status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelRepoEvolutionPlanStatus {
    Accepted,
    Blocked,
    FallbackUsed,
}

/// Repo-local Steel evolution plan reason code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelRepoEvolutionPlanReason {
    Accepted,
    InvalidSchema,
    MalformedPayload,
    UnknownHostCall,
    UnknownGate,
    EmptyActions,
}

/// Patch payload format for Steel self-mutation requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationPatchFormat {
    UnifiedDiff,
    FullReplace,
}

/// UCAN expiry status for Steel self-mutation grants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationUcanExpiryStatus {
    Valid,
    Expired,
}

/// Steel self-mutation authorization decision outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationDecisionOutcome {
    Allowed,
    Denied,
}

/// Steel self-mutation authorization reason code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationReasonCode {
    Allowed,
    InvalidPolicy,
    UnknownTargetClass,
    UnknownVerb,
    VerbNotAllowedForTarget,
    PathEscape,
    DeniedPathPattern,
    MissingPatch,
    MissingApproval,
    ApprovalTierMismatch,
    MissingUcan,
    ExpiredUcan,
    RevokedUcan,
    WrongUcanAbility,
    WrongUcanAudience,
    WrongUcanResource,
    WildcardUcanResource,
    OverDelegatedUcan,
}

/// Steel self-mutation host preflight status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationHostPreflightStatus {
    Ready,
    Denied,
    Blocked,
}

/// Steel self-mutation host preflight reason code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationHostPreflightReason {
    Ready,
    DecisionDenied,
    MissingSessionCapability,
    DisabledHostFunction,
    DirtyRepositoryNeedsCheckpoint,
    MissingTargetHash,
}

/// Steel self-mutation apply status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationApplyStatus {
    Applied,
    Blocked,
    FailedVerification,
    FailedWrite,
}

/// Steel self-mutation apply reason code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationApplyReason {
    Applied,
    PreflightNotReady,
    MissingPatchDescriptor,
    PatchFormatMismatch,
    PatchHashMismatch,
    PatchSizeMismatch,
    UnsupportedPatchFormat,
    StaleTargetHash,
    TargetReadFailed,
    TargetWriteFailed,
    VerificationFailed,
}

/// Steel self-mutation verification status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationVerificationStatus {
    Passed,
    Failed,
    Skipped,
}

/// Steel self-mutation rollback status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationRollbackStatus {
    RolledBack,
    Blocked,
    FailedWrite,
}

/// Steel self-mutation rollback reason code.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelMutationRollbackReason {
    RolledBack,
    ApplyReceiptNotRollbackable,
    MissingRecordedPostApplyHash,
    MissingBackupHash,
    BackupHashMismatch,
    CurrentTargetChanged,
    TargetReadFailed,
    TargetWriteFailed,
}

/// Repo-local Steel orchestration-pack mutation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelOrchestrationMutationStatus {
    Ready,
    Staged,
    Promoted,
    RolledBack,
    Denied,
    FailedValidation,
}

/// Repo-local Steel orchestration-pack mutation reason code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SteelOrchestrationMutationReason {
    Ready,
    Staged,
    Promoted,
    RolledBack,
    InvalidSchema,
    MalformedPatchHash,
    PathEscape,
    StalePackHash,
    RawHostWriteDenied,
    AuthorityKernelChange,
    RequiredGateRemoval,
    UnknownActivationPolicy,
    GateFailed,
    CurrentPackChanged,
    BackupHashMismatch,
    ApplyFailed,
    RollbackFailed,
}

/// Repo-local Steel orchestration-pack activation decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelOrchestrationActivationDecision {
    Denied,
    StagedOnly,
    NextTurn,
    ExplicitReload,
}

/// Policy for handling tool-name collisions while building a tool catalog.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCollisionPolicy {
    #[default]
    Reject,
    KeepExisting,
    HostOverrides,
}

/// High-level side-effect class for tool descriptors.
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

    #[must_use]
    pub fn default_effect_class(self) -> EffectAbilityClass {
        match self {
            Self::ReadOnly | Self::WorkspaceMutation => EffectAbilityClass::Filesystem,
            Self::ExternalIo => EffectAbilityClass::Network,
            Self::Dangerous => EffectAbilityClass::Tool,
        }
    }
}

/// Extension execution status returned by host extension adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExtensionStatus {
    Succeeded,
    Failed,
    Disabled,
    Unavailable,
}

/// Runtime tool execution status returned by host tool adapters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeToolStatus {
    Succeeded,
    Failed,
    Missing,
    Denied,
    Cancelled,
}

/// Runtime tool response returned by host tool adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeToolResponse {
    pub status: RuntimeToolStatus,
    #[serde(default)]
    pub content: Vec<Content>,
    #[serde(default)]
    pub details: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl RuntimeToolResponse {
    #[must_use]
    pub fn succeeded(content: Vec<Content>, details: Value) -> Self {
        Self {
            status: RuntimeToolStatus::Succeeded,
            content,
            details,
            message: None,
        }
    }

    #[must_use]
    pub fn failed(message: impl Into<String>) -> Self {
        let message = message.into();
        Self {
            status: RuntimeToolStatus::Failed,
            content: vec![Content::Text { text: message.clone() }],
            details: Value::Null,
            message: Some(message),
        }
    }
}

/// Runtime retry request passed to host retry adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeRetryRequest {
    pub request_id: String,
    pub delay_ms: u64,
}

impl RuntimeRetryRequest {
    #[must_use]
    pub fn new(request_id: impl Into<String>, delay: Duration) -> Self {
        Self {
            request_id: request_id.into(),
            delay_ms: runtime_retry_delay_ms(delay),
        }
    }
}

fn runtime_retry_delay_ms(delay: Duration) -> u64 {
    let delay_ms = delay.as_millis();
    let delay_ms_max = u128::from(RUNTIME_RETRY_DELAY_MS_MAX);
    if delay_ms > delay_ms_max {
        return RUNTIME_RETRY_DELAY_MS_MAX;
    }
    match u64::try_from(delay_ms) {
        Ok(value) => value,
        Err(_) => RUNTIME_RETRY_DELAY_MS_MAX,
    }
}

/// Runtime usage observation emitted by model/streaming adapters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeUsageObservation {
    pub kind: RuntimeUsageObservationKind,
    pub usage: Usage,
}

/// Kind of runtime usage observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeUsageObservationKind {
    StreamDelta,
    FinalSummary,
}

/// Token usage statistics for a completion.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: usize,
    pub output_tokens: usize,
    pub cache_creation_input_tokens: usize,
    pub cache_read_input_tokens: usize,
}

impl Usage {
    #[cfg_attr(
        dylint_lib = "tigerstyle",
        allow(
            tigerstyle::usize_in_public_api,
            reason = "Usage token counts mirror existing usize fields and internal UI metrics."
        )
    )]
    pub fn total_tokens(&self) -> usize {
        self.input_tokens.saturating_add(self.output_tokens)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_info_defaults_missing_source_for_legacy_wire_events() {
        let info: ToolInfo = serde_json::from_str(r#"{"name":"read","description":"Read files"}"#)
            .expect("tool info should deserialize");
        assert_eq!(info.source, "");
    }

    #[test]
    fn tool_catalog_omission_sanitizes_safe_reason_contract() {
        let omission = ToolCatalogOmission::new("secret-token-tool", "disabled by policy");
        assert_eq!(omission.name, "[REDACTED]");
        assert_eq!(omission.reason, "disabled by policy");
        let json = serde_json::to_string(&omission).expect("tool catalog omission should serialize");
        let parsed: ToolCatalogOmission =
            serde_json::from_str(&json).expect("tool catalog omission should deserialize");
        assert_eq!(parsed, omission);
    }

    #[test]
    fn serialized_message_roundtrip_preserves_optional_fields() {
        let message = SerializedMessage {
            role: "assistant".to_string(),
            content: "hello".to_string(),
            model: Some("model".to_string()),
            timestamp: None,
        };
        let json = serde_json::to_string(&message).expect("message should serialize");
        let parsed: SerializedMessage = serde_json::from_str(&json).expect("message should deserialize");
        assert_eq!(parsed, message);
    }

    #[test]
    fn session_key_matrix_dir_name_sanitizes() {
        let key = SessionKey::Matrix {
            user_id: "@alice:matrix.org".to_string(),
            room_id: "!room123:matrix.org".to_string(),
        };
        let dir = key.dir_name();
        assert!(!dir.contains('@'));
        assert!(!dir.contains(':'));
        assert!(!dir.contains('!'));
        assert!(dir.starts_with("daemon_matrix_"));
    }

    #[test]
    fn session_key_roundtrip_preserves_matrix_identity() {
        let key = SessionKey::Matrix {
            user_id: "@user:host".to_string(),
            room_id: "!room:host".to_string(),
        };
        let json = serde_json::to_string(&key).expect("key should serialize");
        let parsed: SessionKey = serde_json::from_str(&json).expect("key should deserialize");
        assert_eq!(parsed, key);
        assert_eq!(parsed.matrix_room_id(), Some("!room:host"));
    }

    #[test]
    fn session_summary_defaults_missing_state_for_legacy_wire_events() {
        let summary: SessionSummary = serde_json::from_str(
            r#"{"session_id":"s1","model":"model","turn_count":2,"last_active":"now","client_count":1,"socket_path":"/tmp/sock"}"#,
        )
        .expect("summary should deserialize");
        assert_eq!(summary.state, "active");
    }

    #[test]
    fn daemon_status_roundtrip_preserves_counters() {
        let status = DaemonStatus {
            uptime_secs: 4.5,
            session_count: 2,
            total_clients: 3,
            pid: 42,
        };
        let json = serde_json::to_string(&status).expect("status should serialize");
        let parsed: DaemonStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, status);
    }

    #[test]
    fn provider_message_tool_result_preserves_call_metadata() {
        let message = ProviderMessage::tool_result(
            "call-1",
            "read",
            vec![Content::Text {
                text: "result".to_string(),
            }],
            true,
        );
        assert_eq!(message.role, ProviderMessageRole::Tool);
        assert_eq!(message.call_id.as_deref(), Some("call-1"));
        assert_eq!(message.tool_name.as_deref(), Some("read"));
        assert!(message.is_error);
    }

    #[test]
    fn provider_stream_event_usage_roundtrip_preserves_snake_case_type() {
        let event = ProviderStreamEvent::Usage {
            stop_reason: Some(crate::content::StopReason::Stop),
            usage: Usage {
                input_tokens: 1,
                output_tokens: 2,
                cache_creation_input_tokens: 3,
                cache_read_input_tokens: 4,
            },
        };
        let json = serde_json::to_string(&event).expect("event should serialize");
        assert!(json.contains(r#""type":"usage""#));
        let parsed: ProviderStreamEvent = serde_json::from_str(&json).expect("event should deserialize");
        assert!(matches!(parsed, ProviderStreamEvent::Usage {
            stop_reason: Some(crate::content::StopReason::Stop),
            ..
        }));
    }

    #[test]
    fn provider_model_failure_helpers_sanitize_and_mark_retryability() {
        let retryable = ProviderModelFailure::retryable("bearer token leaked", Some(429));
        assert_eq!(retryable.message, "[REDACTED]");
        assert_eq!(retryable.status, Some(429));
        assert!(retryable.retryable);

        let terminal = ProviderModelFailure::terminal("permanent failure", Some(400));
        assert_eq!(terminal.message, "permanent failure");
        assert_eq!(terminal.status, Some(400));
        assert!(!terminal.retryable);
    }

    #[test]
    fn provider_model_status_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ProviderModelStatus::RetryableFailure).expect("status should serialize");
        assert_eq!(json, r#""retryable_failure""#);
        let parsed: ProviderModelStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, ProviderModelStatus::RetryableFailure);
    }

    #[test]
    fn confirmation_action_custom_roundtrip_preserves_payload() {
        let action = ConfirmationAction::Custom("deploy".to_string());
        let json = serde_json::to_string(&action).expect("action should serialize");
        let parsed: ConfirmationAction = serde_json::from_str(&json).expect("action should deserialize");
        assert_eq!(parsed, action);
    }

    #[test]
    fn confirmation_decision_helpers_sanitize_secret_reasons() {
        let approved = ConfirmationDecision::approve("visible");
        assert!(approved.approved);
        assert_eq!(approved.reason, "visible");

        let denied = ConfirmationDecision::deny("bearer token leaked");
        assert!(!denied.approved);
        assert_eq!(denied.reason, "[REDACTED]");
    }

    #[test]
    fn error_class_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ErrorClass::InvalidInput).expect("class should serialize");
        assert_eq!(json, r#""invalid_input""#);
        let parsed: ErrorClass = serde_json::from_str(&json).expect("class should deserialize");
        assert_eq!(parsed, ErrorClass::InvalidInput);
    }

    #[test]
    fn auth_store_operation_roundtrip_preserves_snake_case() {
        let json =
            serde_json::to_string(&AuthStoreOperation::PendingLoginVerifier).expect("operation should serialize");
        assert_eq!(json, r#""pending_login_verifier""#);
        let parsed: AuthStoreOperation = serde_json::from_str(&json).expect("operation should deserialize");
        assert_eq!(parsed, AuthStoreOperation::PendingLoginVerifier);
    }

    #[test]
    fn skill_resolution_request_roundtrip_preserves_requested_order() {
        let request = SkillResolutionRequest {
            requested: vec!["rust".to_string(), "review".to_string()],
        };
        let json = serde_json::to_string(&request).expect("request should serialize");
        let parsed: SkillResolutionRequest = serde_json::from_str(&json).expect("request should deserialize");
        assert_eq!(parsed.requested, vec!["rust", "review"]);
    }

    #[test]
    fn resolved_skill_snippet_roundtrip_preserves_source() {
        let snippet = ResolvedSkillSnippet {
            name: "rust".to_string(),
            description: "Rust instructions".to_string(),
            content: "Prefer focused tests".to_string(),
            source: "host".to_string(),
        };
        let json = serde_json::to_string(&snippet).expect("snippet should serialize");
        let parsed: ResolvedSkillSnippet = serde_json::from_str(&json).expect("snippet should deserialize");
        assert_eq!(parsed, snippet);
    }

    #[test]
    fn model_request_metadata_roundtrip_preserves_generation_settings() {
        let metadata = ModelRequestMetadata {
            request_id: "req-1".to_string(),
            message_count: 3,
            system_prompt: "system".to_string(),
            max_tokens: Some(1024),
            temperature: Some(0.2),
            tool_names: vec!["read".to_string(), "write".to_string()],
            no_cache: true,
            cache_ttl: Some("30m".to_string()),
        };
        let json = serde_json::to_string(&metadata).expect("metadata should serialize");
        let parsed: ModelRequestMetadata = serde_json::from_str(&json).expect("metadata should deserialize");
        assert_eq!(parsed, metadata);
    }

    #[test]
    fn model_failure_helpers_preserve_retryability() {
        let retryable = ModelFailure::retryable("rate limited", Some(429));
        assert!(retryable.retryable);
        assert_eq!(retryable.status, Some(429));

        let terminal = ModelFailure::terminal("bad request", Some(400));
        assert!(!terminal.retryable);
        assert_eq!(terminal.message, "bad request");
    }

    #[test]
    fn prompt_assembly_policy_preserves_host_defaults() {
        let host_only = PromptAssemblyPolicy::host_context_only();
        assert!(!host_only.allow_filesystem_discovery);
        assert!(!host_only.context_references_enabled);

        let desktop = PromptAssemblyPolicy::desktop_default();
        assert!(desktop.allow_filesystem_discovery);
        assert!(desktop.context_references_enabled);
    }

    #[test]
    fn prompt_sources_roundtrip_preserves_context_references_and_defaults() {
        let sources = PromptSources {
            system_prompt: Some("system".to_string()),
            host_context: vec![HostContext {
                label: "host".to_string(),
                content: "context".to_string(),
            }],
            filesystem_context_requested: true,
            context_references: vec![ContextReferenceRequest::new("README.md", ContextReferenceKind::File)],
            skill_snippets: vec![SkillSnippet {
                name: "rust".to_string(),
                content: "Prefer focused tests".to_string(),
                source: "host".to_string(),
            }],
            ..PromptSources::default()
        };
        let json = serde_json::to_string(&sources).expect("sources should serialize");
        let parsed: PromptSources = serde_json::from_str(&json).expect("sources should deserialize");
        assert_eq!(parsed.context_references[0].kind, ContextReferenceKind::File);
        assert_eq!(parsed.skill_snippets[0].source, "host");
        assert!(parsed.filesystem_context.is_empty());
    }

    #[test]
    fn assembled_prompt_roundtrip_preserves_provenance_and_unsupported_refs() {
        let prompt = AssembledPrompt {
            user_prompt: "hello".to_string(),
            sections: vec![PromptSection {
                label: "Host".to_string(),
                content: "context".to_string(),
            }],
            provenance: vec![PromptProvenance {
                label: "Host".to_string(),
                source: PromptSourceKind::Host,
                safe_summary: "1 block".to_string(),
            }],
            context_references_enabled: false,
            unsupported_context_references: vec![UnsupportedContextReference {
                label: "README.md".to_string(),
                kind: ContextReferenceKind::File,
                reason: "disabled".to_string(),
            }],
        };
        let json = serde_json::to_string(&prompt).expect("assembled prompt should serialize");
        let parsed: AssembledPrompt = serde_json::from_str(&json).expect("assembled prompt should deserialize");
        assert_eq!(parsed, prompt);
    }

    #[test]
    fn prompt_source_kind_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&PromptSourceKind::Filesystem).expect("source should serialize");
        assert_eq!(json, r#""filesystem""#);
        let parsed: PromptSourceKind = serde_json::from_str(&json).expect("source should deserialize");
        assert_eq!(parsed, PromptSourceKind::Filesystem);
    }

    #[test]
    fn extension_runtime_kind_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ExtensionRuntimeKind::Mcp).expect("kind should serialize");
        assert_eq!(json, r#""mcp""#);
        let parsed: ExtensionRuntimeKind = serde_json::from_str(&json).expect("kind should deserialize");
        assert_eq!(parsed, ExtensionRuntimeKind::Mcp);
    }

    #[test]
    fn auth_store_access_request_roundtrip_preserves_operation() {
        let request = AuthStoreAccessRequest {
            provider: "openai-codex".to_string(),
            account_label: Some("work".to_string()),
            operation: AuthStoreOperation::RefreshPersist,
        };
        let json = serde_json::to_string(&request).expect("request should serialize");
        assert!(json.contains("refresh_persist"));
        let parsed: AuthStoreAccessRequest = serde_json::from_str(&json).expect("request should deserialize");
        assert_eq!(parsed, request);
    }

    #[test]
    fn credential_pool_request_roundtrip_preserves_strategy() {
        let request = CredentialPoolRequest {
            provider: "anthropic".to_string(),
            strategy: "least_recently_used".to_string(),
            account_label: None,
        };
        let json = serde_json::to_string(&request).expect("request should serialize");
        let parsed: CredentialPoolRequest = serde_json::from_str(&json).expect("request should deserialize");
        assert_eq!(parsed.strategy, "least_recently_used");
        assert_eq!(parsed, request);
    }

    #[test]
    fn extension_runtime_request_defaults_missing_arguments_to_null() {
        let parsed: ExtensionRuntimeRequest = serde_json::from_str(
            r#"{"kind":"plugin","action":"call","extension_name":null,"visible_tool_name":"demo","original_tool_name":null,"runtime_entrypoint":null}"#,
        )
        .expect("request should deserialize");
        assert_eq!(parsed.kind, ExtensionRuntimeKind::Plugin);
        assert!(parsed.arguments.is_null());
    }

    #[test]
    fn effect_ability_class_roundtrip_preserves_kebab_case() {
        let json = serde_json::to_string(&EffectAbilityClass::Filesystem).expect("class should serialize");
        assert_eq!(json, r#""filesystem""#);
        let parsed: EffectAbilityClass = serde_json::from_str(&json).expect("class should deserialize");
        assert_eq!(parsed, EffectAbilityClass::Filesystem);
    }

    #[test]
    fn effect_result_status_roundtrip_preserves_kebab_case() {
        let json = serde_json::to_string(&EffectResultStatus::Unavailable).expect("status should serialize");
        assert_eq!(json, r#""unavailable""#);
        let parsed: EffectResultStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, EffectResultStatus::Unavailable);
    }

    #[test]
    fn effect_correlation_id_is_stable_for_replay_and_serialization() {
        let correlation_id = EffectCorrelationId::from_string("effect-static-1");
        assert_eq!(correlation_id.as_str(), "effect-static-1");
        let json = serde_json::to_string(&correlation_id).expect("correlation id should serialize");
        assert_eq!(json, r#""effect-static-1""#);
        let parsed: EffectCorrelationId = serde_json::from_str(&json).expect("correlation id should deserialize");
        assert_eq!(parsed, correlation_id);
    }

    #[test]
    fn effect_request_and_result_receipts_sanitize_metadata_contracts() {
        let schema_hash = ArtifactHash::digest(b"schema");
        let dep_a = ArtifactHash::digest(b"a");
        let dep_b = ArtifactHash::digest(b"b");
        let request = EffectRequest::new(
            EffectAbilityClass::Shell,
            EffectCorrelationId::from_static("effect-1"),
            RedactionClass::MetadataOnly,
        )
        .with_input_schema_hash(schema_hash)
        .with_artifact_dependencies([dep_b, dep_a, dep_b])
        .with_safe_metadata("tool", "bash")
        .with_safe_metadata("authorization", "Bearer secret-token");

        let mut expected_dependencies = vec![dep_a, dep_b];
        expected_dependencies.sort_by_key(|hash| hash.hex());
        assert_eq!(request.declared_artifact_dependencies, expected_dependencies);
        assert_eq!(request.safe_source_metadata.get("tool"), Some(&"bash".to_owned()));
        let metadata_json = serde_json::to_string(&request.safe_source_metadata).expect("metadata json");
        assert!(!metadata_json.contains("Bearer"));
        assert!(!metadata_json.contains("secret-token"));
        assert!(metadata_json.contains("redacted"));

        let result = EffectResult::new(&request, EffectResultStatus::Denied, "token secret value denied")
            .with_output_artifact(schema_hash);
        assert_eq!(result.request.class, EffectAbilityClass::Shell);
        assert_eq!(result.request.correlation_id.as_str(), "effect-1");
        assert_eq!(result.status, EffectResultStatus::Denied);
        assert_eq!(result.safe_summary, "[redacted-secret-marker]");
        assert_eq!(result.output_artifact, Some(schema_hash));
        let result_json = serde_json::to_string(&result).expect("effect result should serialize");
        let parsed: EffectResult = serde_json::from_str(&result_json).expect("effect result should deserialize");
        assert_eq!(parsed, result);
    }

    #[test]
    fn ucan_authorization_metadata_redacts_and_sorts_safe_receipt_fields() {
        let token_like = format!("ey{}.{}.{}", "a".repeat(90), "b".repeat(4), "c".repeat(4));
        let metadata = UcanAuthorizationMetadata::new("file/read", token_like, "allowed")
            .with_issuer("did:key:zissuer")
            .with_audience("bearer secret audience")
            .with_proof_references(["proof-b", "proof-a", "proof-a"])
            .with_caveat_ids(["write", "read"])
            .with_replay_status("fresh")
            .with_revocation_status("not_revoked")
            .with_denial_class("none");
        assert_eq!(metadata.ability, "file/read");
        assert_eq!(metadata.resource_uri, "[redacted-secret-marker]");
        assert_eq!(metadata.audience.as_deref(), Some("[redacted-secret-marker]"));
        assert_eq!(metadata.proof_references, vec!["proof-a".to_string(), "proof-b".to_string()]);
        assert_eq!(metadata.caveat_ids, vec!["read".to_string(), "write".to_string()]);
        let json = serde_json::to_string(&metadata).expect("UCAN metadata should serialize");
        let parsed: UcanAuthorizationMetadata = serde_json::from_str(&json).expect("UCAN metadata should deserialize");
        assert_eq!(parsed, metadata);
    }

    #[test]
    fn remote_execution_selectors_roundtrip_preserve_kebab_case() {
        let artifact =
            serde_json::to_string(&RemoteExecutionArtifactKind::ToolSchema).expect("artifact kind should serialize");
        assert_eq!(artifact, r#""tool-schema""#);
        let parsed_artifact: RemoteExecutionArtifactKind =
            serde_json::from_str(&artifact).expect("artifact kind should deserialize");
        assert_eq!(parsed_artifact, RemoteExecutionArtifactKind::ToolSchema);

        let target = serde_json::to_string(&RemoteExecutionTarget::RemoteDaemon).expect("target should serialize");
        assert_eq!(target, r#""remote-daemon""#);
        let parsed_target: RemoteExecutionTarget = serde_json::from_str(&target).expect("target should deserialize");
        assert_eq!(parsed_target, RemoteExecutionTarget::RemoteDaemon);
    }

    #[test]
    fn remote_execution_dependency_and_envelope_contracts_are_stable() {
        let prompt_hash = ArtifactHash::digest(b"prompt");
        let skill_hash = ArtifactHash::digest(b"skill");
        let prompt = RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, prompt_hash);
        let skill = RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Skill, skill_hash);
        let request =
            RemoteExecutionRequest::new(RemoteExecutionTarget::Subagent, EffectCorrelationId::from_static("r1"))
                .with_required_artifacts([skill.clone(), prompt.clone(), prompt.clone()]);
        let mut expected_hashes = vec![prompt_hash, skill_hash];
        expected_hashes.sort_by_key(|hash| hash.hex());
        assert_eq!(request.required_hashes(), expected_hashes);
        assert_eq!(request.required_artifacts, vec![prompt.clone(), skill.clone()]);

        let envelope = RemoteArtifactEnvelope::new(
            prompt.clone(),
            REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION,
            prompt_hash,
            RedactionClass::Public,
        )
        .with_ucan_authorization(UcanAuthorizationMetadata::new(
            "artifact/read",
            "artifact://prompt",
            "allowed",
        ));
        let json = serde_json::to_string(&envelope).expect("remote artifact envelope should serialize");
        let parsed: RemoteArtifactEnvelope =
            serde_json::from_str(&json).expect("remote artifact envelope should deserialize");
        assert_eq!(parsed, envelope);

        let failure = RemoteDependencyFailure::new(
            skill,
            RemoteDependencyFailureKind::SecretDependencyDenied,
            "secret token leaked",
        );
        assert_eq!(failure.safe_summary, "[REDACTED]");
        let failure_json = serde_json::to_string(&failure).expect("remote dependency failure should serialize");
        let parsed_failure: RemoteDependencyFailure =
            serde_json::from_str(&failure_json).expect("remote dependency failure should deserialize");
        assert_eq!(parsed_failure, failure);

        let sync_report = RemoteDependencySyncReport {
            missing_safe_artifacts: vec![prompt],
            failures: vec![failure],
        };
        assert!(!sync_report.ready());
        let effect_request = EffectRequest::new(
            EffectAbilityClass::Provider,
            EffectCorrelationId::from_static("remote-sync"),
            RedactionClass::MetadataOnly,
        );
        let effect_result = sync_report.to_effect_result(&effect_request);
        assert_eq!(effect_result.status, EffectResultStatus::Unavailable);
        assert_eq!(effect_result.safe_summary, "remote dependencies unavailable");
    }

    #[test]
    fn dynamic_runtime_selector_status_dtos_roundtrip_preserve_snake_case() {
        let runtime = serde_json::to_string(&DynamicRuntimeKind::SteelScheme).expect("runtime kind should serialize");
        assert_eq!(runtime, r#""steel_scheme""#);
        let parsed_runtime: DynamicRuntimeKind =
            serde_json::from_str(&runtime).expect("runtime kind should deserialize");
        assert_eq!(parsed_runtime, DynamicRuntimeKind::SteelScheme);

        let action =
            serde_json::to_string(&DynamicRuntimeActionKind::HostFunction).expect("action kind should serialize");
        assert_eq!(action, r#""host_function""#);
        let parsed_action: DynamicRuntimeActionKind =
            serde_json::from_str(&action).expect("action kind should deserialize");
        assert_eq!(parsed_action, DynamicRuntimeActionKind::HostFunction);

        let redaction = serde_json::to_string(&DynamicRuntimeRedactionClass::MetadataOnly)
            .expect("redaction class should serialize");
        assert_eq!(redaction, r#""metadata_only""#);
        let parsed_redaction: DynamicRuntimeRedactionClass =
            serde_json::from_str(&redaction).expect("redaction class should deserialize");
        assert_eq!(parsed_redaction, DynamicRuntimeRedactionClass::MetadataOnly);

        let status = serde_json::to_string(&DynamicRuntimeActionStatus::UcanDenied).expect("status should serialize");
        assert_eq!(status, r#""ucan_denied""#);
        let parsed_status: DynamicRuntimeActionStatus =
            serde_json::from_str(&status).expect("status should deserialize");
        assert_eq!(parsed_status, DynamicRuntimeActionStatus::UcanDenied);

        let reason =
            serde_json::to_string(&DynamicRuntimeActionReason::UnsafeTargetResource).expect("reason should serialize");
        assert_eq!(reason, r#""unsafe_target_resource""#);
        let parsed_reason: DynamicRuntimeActionReason =
            serde_json::from_str(&reason).expect("reason should deserialize");
        assert_eq!(parsed_reason, DynamicRuntimeActionReason::UnsafeTargetResource);

        let ambient =
            serde_json::to_string(&SteelAmbientAccessKind::NativeTool).expect("ambient access kind should serialize");
        assert_eq!(ambient, r#""native_tool""#);
        let parsed_ambient: SteelAmbientAccessKind =
            serde_json::from_str(&ambient).expect("ambient access kind should deserialize");
        assert_eq!(parsed_ambient, SteelAmbientAccessKind::NativeTool);
        assert_eq!(parsed_ambient.host_function_name(), "steel.ambient.native_tool");
        assert_eq!(parsed_ambient.target_resource(), "native-tool:ambient");
        assert_eq!(parsed_ambient.route_hint(), "native tool");

        let wasm = serde_json::to_string(&WasmToolExecutionStatus::Completed).expect("wasm status should serialize");
        assert_eq!(wasm, r#""completed""#);
        let parsed_wasm: WasmToolExecutionStatus = serde_json::from_str(&wasm).expect("wasm status should deserialize");
        assert_eq!(parsed_wasm, WasmToolExecutionStatus::Completed);

        let profile = SteelRuntimeProfile::default_deny();
        assert_eq!(profile.name, "default-deny");
        assert!(!profile.ambient_authority);
        let request = SteelRuntimeRequest::pure("(host \"demo\")");
        assert_eq!(request.receipt_destination, "stdout");
        assert!(request.host_functions.is_empty());
        let status_dto = SteelRuntimeStatus {
            schema: "clankers.steel_runtime.status.v1".to_string(),
            available: true,
            implementation: "fixture".to_string(),
            profile: profile.clone(),
            agent_tool_enabled: false,
            ambient_authority: false,
            sandbox_claim: "none".to_string(),
        };
        let status_json = serde_json::to_string(&status_dto).expect("Steel runtime status DTO should serialize");
        let parsed_status_dto: SteelRuntimeStatus =
            serde_json::from_str(&status_json).expect("Steel runtime status DTO should deserialize");
        assert_eq!(parsed_status_dto, status_dto);
        let registration = SteelHostFunctionRegistration {
            name: "steel.host.demo".to_string(),
            required_capability: "demo".to_string(),
            output: "ok".to_string(),
        };
        let registration_json =
            serde_json::to_string(&registration).expect("Steel host function registration should serialize");
        let parsed_registration: SteelHostFunctionRegistration =
            serde_json::from_str(&registration_json).expect("Steel host function registration should deserialize");
        assert_eq!(parsed_registration, registration);

        let runtime_status = serde_json::to_string(&SteelRuntimeStatusCode::ResourceLimited)
            .expect("Steel runtime status should serialize");
        assert_eq!(runtime_status, r#""resource_limited""#);
        let parsed_runtime_status: SteelRuntimeStatusCode =
            serde_json::from_str(&runtime_status).expect("Steel runtime status should deserialize");
        assert_eq!(parsed_runtime_status, SteelRuntimeStatusCode::ResourceLimited);

        let runtime_reason = serde_json::to_string(&SteelRuntimeReasonCode::MissingHostCapability)
            .expect("Steel runtime reason should serialize");
        assert_eq!(runtime_reason, r#""missing-host-capability""#);
        let parsed_runtime_reason: SteelRuntimeReasonCode =
            serde_json::from_str(&runtime_reason).expect("Steel runtime reason should deserialize");
        assert_eq!(parsed_runtime_reason, SteelRuntimeReasonCode::MissingHostCapability);

        let host_call_outcome =
            serde_json::to_string(&SteelHostCallOutcome::Approved).expect("Steel host-call outcome should serialize");
        assert_eq!(host_call_outcome, r#""approved""#);
        let parsed_host_call_outcome: SteelHostCallOutcome =
            serde_json::from_str(&host_call_outcome).expect("Steel host-call outcome should deserialize");
        assert_eq!(parsed_host_call_outcome, SteelHostCallOutcome::Approved);
        let host_call_receipt = SteelHostCallReceipt {
            name: "steel.host.demo".to_string(),
            outcome: SteelHostCallOutcome::Approved,
            safe_message: "approved".to_string(),
        };
        let receipt_json = serde_json::to_string(&host_call_receipt).expect("host-call receipt should serialize");
        let parsed_receipt: SteelHostCallReceipt =
            serde_json::from_str(&receipt_json).expect("host-call receipt should deserialize");
        assert_eq!(parsed_receipt, host_call_receipt);

        let executor =
            serde_json::to_string(&SteelToolExecutorKind::StdioPlugin).expect("executor kind should serialize");
        assert_eq!(executor, r#""stdio_plugin""#);
        let parsed_executor: SteelToolExecutorKind =
            serde_json::from_str(&executor).expect("executor kind should deserialize");
        assert_eq!(parsed_executor, SteelToolExecutorKind::StdioPlugin);
        assert_eq!(parsed_executor.as_str(), "stdio_plugin");
        assert_eq!(SteelToolExecutorKind::parse("stdio_plugin"), Some(parsed_executor));

        let rollout =
            serde_json::to_string(&SteelToolSubstrateRolloutStage::Comparison).expect("rollout stage should serialize");
        assert_eq!(rollout, r#""comparison""#);
        let parsed_rollout: SteelToolSubstrateRolloutStage =
            serde_json::from_str(&rollout).expect("rollout stage should deserialize");
        assert_eq!(parsed_rollout, SteelToolSubstrateRolloutStage::Comparison);

        let fallback =
            serde_json::to_string(&SteelToolSubstrateFallbackMode::RustNative).expect("fallback mode should serialize");
        assert_eq!(fallback, r#""rust_native""#);
        let parsed_fallback: SteelToolSubstrateFallbackMode =
            serde_json::from_str(&fallback).expect("fallback mode should deserialize");
        assert_eq!(parsed_fallback, SteelToolSubstrateFallbackMode::RustNative);

        let substrate_status =
            serde_json::to_string(&SteelToolSubstrateStatus::FallbackUsed).expect("substrate status should serialize");
        assert_eq!(substrate_status, r#""fallback_used""#);
        let parsed_substrate_status: SteelToolSubstrateStatus =
            serde_json::from_str(&substrate_status).expect("substrate status should deserialize");
        assert_eq!(parsed_substrate_status, SteelToolSubstrateStatus::FallbackUsed);

        let issue = serde_json::to_string(&SteelToolSubstrateIssue::ExecutorKindDenied)
            .expect("substrate issue should serialize");
        assert_eq!(issue, r#""executor-kind-denied""#);
        let parsed_issue: SteelToolSubstrateIssue =
            serde_json::from_str(&issue).expect("substrate issue should deserialize");
        assert_eq!(parsed_issue, SteelToolSubstrateIssue::ExecutorKindDenied);

        let orchestration_rollout =
            serde_json::to_string(&OrchestrationRolloutStage::Default).expect("orchestration rollout should serialize");
        assert_eq!(orchestration_rollout, r#""default""#);
        let parsed_orchestration_rollout: OrchestrationRolloutStage =
            serde_json::from_str(&orchestration_rollout).expect("orchestration rollout should deserialize");
        assert_eq!(parsed_orchestration_rollout, OrchestrationRolloutStage::Default);

        let orchestration_fallback = serde_json::to_string(&OrchestrationFallbackMode::RustNative)
            .expect("orchestration fallback should serialize");
        assert_eq!(orchestration_fallback, r#""rust_native""#);
        let parsed_orchestration_fallback: OrchestrationFallbackMode =
            serde_json::from_str(&orchestration_fallback).expect("orchestration fallback should deserialize");
        assert_eq!(parsed_orchestration_fallback, OrchestrationFallbackMode::RustNative);

        let planner = serde_json::to_string(&OrchestrationPlannerKind::SteelScheme).expect("planner should serialize");
        assert_eq!(planner, r#""steel_scheme""#);
        let parsed_planner: OrchestrationPlannerKind =
            serde_json::from_str(&planner).expect("planner should deserialize");
        assert_eq!(parsed_planner, OrchestrationPlannerKind::SteelScheme);

        let plan_status =
            serde_json::to_string(&OrchestrationPlanStatus::FallbackUsed).expect("plan status should serialize");
        assert_eq!(plan_status, r#""fallback_used""#);
        let parsed_plan_status: OrchestrationPlanStatus =
            serde_json::from_str(&plan_status).expect("plan status should deserialize");
        assert_eq!(parsed_plan_status, OrchestrationPlanStatus::FallbackUsed);

        let issue_code =
            serde_json::to_string(&OrchestrationIssueCode::UcanAuthorityDenied).expect("issue code should serialize");
        assert_eq!(issue_code, r#""ucan-authority-denied""#);
        let parsed_issue_code: OrchestrationIssueCode =
            serde_json::from_str(&issue_code).expect("issue code should deserialize");
        assert_eq!(parsed_issue_code, OrchestrationIssueCode::UcanAuthorityDenied);

        let fallback_status =
            serde_json::to_string(&RustNativeFallbackStatus::Unavailable).expect("fallback status should serialize");
        assert_eq!(fallback_status, r#""unavailable""#);
        let parsed_fallback_status: RustNativeFallbackStatus =
            serde_json::from_str(&fallback_status).expect("fallback status should deserialize");
        assert_eq!(parsed_fallback_status, RustNativeFallbackStatus::Unavailable);

        let authority_status = serde_json::to_string(&SteelTurnPlanningAuthorityStatus::Denied)
            .expect("authority status should serialize");
        assert_eq!(authority_status, r#""denied""#);
        let parsed_authority_status: SteelTurnPlanningAuthorityStatus =
            serde_json::from_str(&authority_status).expect("authority status should deserialize");
        assert_eq!(parsed_authority_status, SteelTurnPlanningAuthorityStatus::Denied);

        let authority_reason = serde_json::to_string(&SteelTurnPlanningAuthorityReason::OverbroadGrant)
            .expect("authority reason should serialize");
        assert_eq!(authority_reason, r#""overbroad-grant""#);
        let parsed_authority_reason: SteelTurnPlanningAuthorityReason =
            serde_json::from_str(&authority_reason).expect("authority reason should deserialize");
        assert_eq!(parsed_authority_reason, SteelTurnPlanningAuthorityReason::OverbroadGrant);

        let execution_status =
            serde_json::to_string(&SteelTurnExecutionStatus::Authorized).expect("execution status should serialize");
        assert_eq!(execution_status, r#""authorized""#);
        let parsed_execution_status: SteelTurnExecutionStatus =
            serde_json::from_str(&execution_status).expect("execution status should deserialize");
        assert_eq!(parsed_execution_status, SteelTurnExecutionStatus::Authorized);

        let repo_fallback = serde_json::to_string(&SteelRepoEvolutionFallbackMode::RustNative)
            .expect("repo evolution fallback should serialize");
        assert_eq!(repo_fallback, r#""rust_native""#);
        let parsed_repo_fallback: SteelRepoEvolutionFallbackMode =
            serde_json::from_str(&repo_fallback).expect("repo evolution fallback should deserialize");
        assert_eq!(parsed_repo_fallback, SteelRepoEvolutionFallbackMode::RustNative);

        let activation_status = serde_json::to_string(&SteelRepoEvolutionActivationStatus::Denied)
            .expect("activation status should serialize");
        assert_eq!(activation_status, r#""denied""#);
        let parsed_activation_status: SteelRepoEvolutionActivationStatus =
            serde_json::from_str(&activation_status).expect("activation status should deserialize");
        assert_eq!(parsed_activation_status, SteelRepoEvolutionActivationStatus::Denied);

        let activation_reason = serde_json::to_string(&SteelRepoEvolutionActivationReason::InvalidHigherOrderContract)
            .expect("activation reason should serialize");
        assert_eq!(activation_reason, r#""invalid-higher-order-contract""#);
        let parsed_activation_reason: SteelRepoEvolutionActivationReason =
            serde_json::from_str(&activation_reason).expect("activation reason should deserialize");
        assert_eq!(parsed_activation_reason, SteelRepoEvolutionActivationReason::InvalidHigherOrderContract);

        let plan_status = serde_json::to_string(&SteelRepoEvolutionPlanStatus::FallbackUsed)
            .expect("repo evolution plan status should serialize");
        assert_eq!(plan_status, r#""fallback_used""#);
        let parsed_plan_status: SteelRepoEvolutionPlanStatus =
            serde_json::from_str(&plan_status).expect("repo evolution plan status should deserialize");
        assert_eq!(parsed_plan_status, SteelRepoEvolutionPlanStatus::FallbackUsed);

        let plan_reason = serde_json::to_string(&SteelRepoEvolutionPlanReason::MalformedPayload)
            .expect("repo evolution plan reason should serialize");
        assert_eq!(plan_reason, r#""malformed-payload""#);
        let parsed_plan_reason: SteelRepoEvolutionPlanReason =
            serde_json::from_str(&plan_reason).expect("repo evolution plan reason should deserialize");
        assert_eq!(parsed_plan_reason, SteelRepoEvolutionPlanReason::MalformedPayload);
    }

    #[test]
    fn dynamic_runtime_action_and_wasm_contracts_roundtrip() {
        let input_hash = ArtifactHash::digest(b"dynamic-input");
        let receipt_hash = ArtifactHash::digest(b"dynamic-receipt");
        let envelope = DynamicRuntimeActionEnvelope {
            schema: DYNAMIC_RUNTIME_ACTION_SCHEMA.to_string(),
            action_id: "steel:demo".to_string(),
            runtime: DynamicRuntimeKind::SteelScheme,
            runtime_profile: "default-deny".to_string(),
            action_kind: DynamicRuntimeActionKind::HostFunction,
            action_name: "steel.host.demo".to_string(),
            target_resource: "host:demo".to_string(),
            receipt_destination: "target/receipts/demo.json".to_string(),
            required_ucan_ability: "host/call".to_string(),
            required_session_capabilities: vec!["host-call".to_string()],
            input_hash,
            input_bytes: 13,
            redaction: DynamicRuntimeRedactionClass::MetadataOnly,
        };
        assert_eq!(envelope.stable_action_key(), "host_function:steel.host.demo");

        let context = DynamicRuntimeAuthorizationContext {
            allowed_runtime_profiles: BTreeSet::from(["default-deny".to_string()]),
            allowed_actions: BTreeSet::from([envelope.stable_action_key()]),
            granted_ucan_abilities: BTreeSet::from(["host/call".to_string()]),
            session_capabilities: BTreeSet::from(["host-call".to_string()]),
            disabled_actions: BTreeSet::new(),
            max_input_bytes: 4096,
        };
        let context_json = serde_json::to_string(&context).expect("dynamic runtime context should serialize");
        let parsed_context: DynamicRuntimeAuthorizationContext =
            serde_json::from_str(&context_json).expect("dynamic runtime context should deserialize");
        assert_eq!(parsed_context, context);

        let receipt = DynamicRuntimeActionReceipt {
            schema: DYNAMIC_RUNTIME_RECEIPT_SCHEMA.to_string(),
            action_id: envelope.action_id.clone(),
            runtime: envelope.runtime,
            runtime_profile: envelope.runtime_profile.clone(),
            action_kind: envelope.action_kind,
            action_name: envelope.action_name.clone(),
            target_resource: envelope.target_resource.clone(),
            receipt_destination: envelope.receipt_destination.clone(),
            status: DynamicRuntimeActionStatus::Allowed,
            reason: DynamicRuntimeActionReason::Ready,
            safe_summary: "authorized".to_string(),
            required_ucan_ability: envelope.required_ucan_ability.clone(),
            required_session_capabilities: envelope.required_session_capabilities.clone(),
            input_hash,
            input_bytes: envelope.input_bytes,
            writes_performed: false,
            receipt_hash,
        };
        let wasm_receipt = WasmToolExecutionReceipt {
            status: WasmToolExecutionStatus::Completed,
            authorization_receipt: receipt.clone(),
            used_imports: vec!["console.log".to_string()],
            memory_pages: 1,
            fuel: 10,
            time_ms: 2,
            output_hash: Some(ArtifactHash::digest(b"dynamic-output")),
        };
        let cross_layer = CrossLayerFixtureReceipt {
            nickel_profile_validated: true,
            steel_route_receipt: FakeSteelOrchestrationReceipt {
                selected_action: envelope,
                authorization_receipt: receipt,
            },
            wasm_execution_receipt: wasm_receipt,
        };
        let json = serde_json::to_string(&cross_layer).expect("dynamic runtime fixture should serialize");
        let parsed: CrossLayerFixtureReceipt =
            serde_json::from_str(&json).expect("dynamic runtime fixture should deserialize");
        assert_eq!(parsed, cross_layer);
    }

    #[test]
    fn steel_mutation_selector_status_dtos_roundtrip_preserve_wire_case() {
        let patch_format = serde_json::to_string(&SteelMutationPatchFormat::FullReplace)
            .expect("mutation patch format should serialize");
        assert_eq!(patch_format, r#""full_replace""#);
        let parsed_patch_format: SteelMutationPatchFormat =
            serde_json::from_str(&patch_format).expect("mutation patch format should deserialize");
        assert_eq!(parsed_patch_format, SteelMutationPatchFormat::FullReplace);

        let expiry_status = serde_json::to_string(&SteelMutationUcanExpiryStatus::Expired)
            .expect("mutation UCAN expiry should serialize");
        assert_eq!(expiry_status, r#""expired""#);
        let parsed_expiry_status: SteelMutationUcanExpiryStatus =
            serde_json::from_str(&expiry_status).expect("mutation UCAN expiry should deserialize");
        assert_eq!(parsed_expiry_status, SteelMutationUcanExpiryStatus::Expired);

        let decision_outcome = serde_json::to_string(&SteelMutationDecisionOutcome::Denied)
            .expect("mutation decision outcome should serialize");
        assert_eq!(decision_outcome, r#""denied""#);
        let parsed_decision_outcome: SteelMutationDecisionOutcome =
            serde_json::from_str(&decision_outcome).expect("mutation decision outcome should deserialize");
        assert_eq!(parsed_decision_outcome, SteelMutationDecisionOutcome::Denied);

        let reason = serde_json::to_string(&SteelMutationReasonCode::ApprovalTierMismatch)
            .expect("mutation reason should serialize");
        assert_eq!(reason, r#""approval-tier-mismatch""#);
        let parsed_reason: SteelMutationReasonCode =
            serde_json::from_str(&reason).expect("mutation reason should deserialize");
        assert_eq!(parsed_reason, SteelMutationReasonCode::ApprovalTierMismatch);

        let preflight_status = serde_json::to_string(&SteelMutationHostPreflightStatus::Blocked)
            .expect("mutation preflight status should serialize");
        assert_eq!(preflight_status, r#""blocked""#);
        let parsed_preflight_status: SteelMutationHostPreflightStatus =
            serde_json::from_str(&preflight_status).expect("mutation preflight status should deserialize");
        assert_eq!(parsed_preflight_status, SteelMutationHostPreflightStatus::Blocked);

        let preflight_reason = serde_json::to_string(&SteelMutationHostPreflightReason::MissingTargetHash)
            .expect("mutation preflight reason should serialize");
        assert_eq!(preflight_reason, r#""missing-target-hash""#);
        let parsed_preflight_reason: SteelMutationHostPreflightReason =
            serde_json::from_str(&preflight_reason).expect("mutation preflight reason should deserialize");
        assert_eq!(parsed_preflight_reason, SteelMutationHostPreflightReason::MissingTargetHash);

        let apply_status = serde_json::to_string(&SteelMutationApplyStatus::FailedVerification)
            .expect("mutation apply status should serialize");
        assert_eq!(apply_status, r#""failed_verification""#);
        let parsed_apply_status: SteelMutationApplyStatus =
            serde_json::from_str(&apply_status).expect("mutation apply status should deserialize");
        assert_eq!(parsed_apply_status, SteelMutationApplyStatus::FailedVerification);

        let apply_reason = serde_json::to_string(&SteelMutationApplyReason::PatchHashMismatch)
            .expect("mutation apply reason should serialize");
        assert_eq!(apply_reason, r#""patch-hash-mismatch""#);
        let parsed_apply_reason: SteelMutationApplyReason =
            serde_json::from_str(&apply_reason).expect("mutation apply reason should deserialize");
        assert_eq!(parsed_apply_reason, SteelMutationApplyReason::PatchHashMismatch);

        let verification_status = serde_json::to_string(&SteelMutationVerificationStatus::Skipped)
            .expect("mutation verification status should serialize");
        assert_eq!(verification_status, r#""skipped""#);
        let parsed_verification_status: SteelMutationVerificationStatus =
            serde_json::from_str(&verification_status).expect("mutation verification status should deserialize");
        assert_eq!(parsed_verification_status, SteelMutationVerificationStatus::Skipped);

        let rollback_status = serde_json::to_string(&SteelMutationRollbackStatus::FailedWrite)
            .expect("mutation rollback status should serialize");
        assert_eq!(rollback_status, r#""failed_write""#);
        let parsed_rollback_status: SteelMutationRollbackStatus =
            serde_json::from_str(&rollback_status).expect("mutation rollback status should deserialize");
        assert_eq!(parsed_rollback_status, SteelMutationRollbackStatus::FailedWrite);

        let rollback_reason = serde_json::to_string(&SteelMutationRollbackReason::BackupHashMismatch)
            .expect("mutation rollback reason should serialize");
        assert_eq!(rollback_reason, r#""backup-hash-mismatch""#);
        let parsed_rollback_reason: SteelMutationRollbackReason =
            serde_json::from_str(&rollback_reason).expect("mutation rollback reason should deserialize");
        assert_eq!(parsed_rollback_reason, SteelMutationRollbackReason::BackupHashMismatch);
    }

    #[test]
    fn steel_orchestration_mutation_selector_status_dtos_roundtrip_preserve_wire_case() {
        let status = serde_json::to_string(&SteelOrchestrationMutationStatus::FailedValidation)
            .expect("orchestration mutation status should serialize");
        assert_eq!(status, r#""failed_validation""#);
        let parsed_status: SteelOrchestrationMutationStatus =
            serde_json::from_str(&status).expect("orchestration mutation status should deserialize");
        assert_eq!(parsed_status, SteelOrchestrationMutationStatus::FailedValidation);

        let reason = serde_json::to_string(&SteelOrchestrationMutationReason::RawHostWriteDenied)
            .expect("orchestration mutation reason should serialize");
        assert_eq!(reason, r#""raw-host-write-denied""#);
        let parsed_reason: SteelOrchestrationMutationReason =
            serde_json::from_str(&reason).expect("orchestration mutation reason should deserialize");
        assert_eq!(parsed_reason, SteelOrchestrationMutationReason::RawHostWriteDenied);

        let decision = serde_json::to_string(&SteelOrchestrationActivationDecision::ExplicitReload)
            .expect("orchestration activation decision should serialize");
        assert_eq!(decision, r#""explicit_reload""#);
        let parsed_decision: SteelOrchestrationActivationDecision =
            serde_json::from_str(&decision).expect("orchestration activation decision should deserialize");
        assert_eq!(parsed_decision, SteelOrchestrationActivationDecision::ExplicitReload);
    }

    #[test]
    fn remote_dependency_failure_kind_roundtrip_preserves_kebab_case() {
        let json = serde_json::to_string(&RemoteDependencyFailureKind::MissingSafeArtifact)
            .expect("failure kind should serialize");
        assert_eq!(json, r#""missing-safe-artifact""#);
        let parsed: RemoteDependencyFailureKind = serde_json::from_str(&json).expect("failure kind should deserialize");
        assert_eq!(parsed, RemoteDependencyFailureKind::MissingSafeArtifact);
    }

    #[test]
    fn tool_collision_policy_default_and_roundtrip_preserve_snake_case() {
        assert_eq!(ToolCollisionPolicy::default(), ToolCollisionPolicy::Reject);
        let json = serde_json::to_string(&ToolCollisionPolicy::HostOverrides).expect("policy should serialize");
        assert_eq!(json, r#""host_overrides""#);
        let parsed: ToolCollisionPolicy = serde_json::from_str(&json).expect("policy should deserialize");
        assert_eq!(parsed, ToolCollisionPolicy::HostOverrides);
    }

    #[test]
    fn side_effect_level_maps_default_confirmation_and_effect_classes() {
        assert!(!SideEffectLevel::ReadOnly.requires_confirmation());
        assert!(SideEffectLevel::WorkspaceMutation.requires_confirmation());
        assert_eq!(SideEffectLevel::ReadOnly.default_effect_class(), EffectAbilityClass::Filesystem);
        assert_eq!(SideEffectLevel::ExternalIo.default_effect_class(), EffectAbilityClass::Network);
        assert_eq!(SideEffectLevel::Dangerous.default_effect_class(), EffectAbilityClass::Tool);
    }

    #[test]
    fn extension_status_roundtrip_preserves_snake_case() {
        let json = serde_json::to_string(&ExtensionStatus::Unavailable).expect("status should serialize");
        assert_eq!(json, r#""unavailable""#);
        let parsed: ExtensionStatus = serde_json::from_str(&json).expect("status should deserialize");
        assert_eq!(parsed, ExtensionStatus::Unavailable);
    }

    #[test]
    fn runtime_tool_response_failed_helper_preserves_message() {
        let response = RuntimeToolResponse::failed("tool unavailable");
        assert_eq!(response.status, RuntimeToolStatus::Failed);
        assert_eq!(response.message.as_deref(), Some("tool unavailable"));
        assert!(matches!(response.content.first(), Some(Content::Text { text }) if text == "tool unavailable"));
    }

    #[test]
    fn runtime_tool_response_roundtrip_preserves_status_and_details() {
        let response = RuntimeToolResponse::succeeded(
            vec![Content::Text {
                text: "done".to_string(),
            }],
            serde_json::json!({"exit_code":0}),
        );
        let json = serde_json::to_string(&response).expect("response should serialize");
        assert!(json.contains("succeeded"));
        let parsed: RuntimeToolResponse = serde_json::from_str(&json).expect("response should deserialize");
        assert_eq!(parsed.status, RuntimeToolStatus::Succeeded);
        assert_eq!(parsed.details["exit_code"], 0);
        assert!(matches!(parsed.content.first(), Some(Content::Text { text }) if text == "done"));
    }

    #[test]
    fn runtime_retry_request_clamps_large_delays() {
        let request = RuntimeRetryRequest::new("retry-1", Duration::from_secs(u64::MAX));
        assert_eq!(request.request_id, "retry-1");
        assert_eq!(request.delay_ms, 365 * 24 * 60 * 60 * 1000);
    }

    #[test]
    fn runtime_retry_request_roundtrip_preserves_delay() {
        let request = RuntimeRetryRequest::new("retry-2", Duration::from_millis(42));
        let json = serde_json::to_string(&request).expect("request should serialize");
        let parsed: RuntimeRetryRequest = serde_json::from_str(&json).expect("request should deserialize");
        assert_eq!(parsed.request_id, "retry-2");
        assert_eq!(parsed.delay_ms, 42);
    }

    #[test]
    fn runtime_usage_observation_roundtrip_preserves_kind_and_usage() {
        let observation = RuntimeUsageObservation {
            kind: RuntimeUsageObservationKind::FinalSummary,
            usage: Usage {
                input_tokens: 3,
                output_tokens: 5,
                cache_creation_input_tokens: 7,
                cache_read_input_tokens: 11,
            },
        };
        let json = serde_json::to_string(&observation).expect("observation should serialize");
        assert!(json.contains("final_summary"));
        let parsed: RuntimeUsageObservation = serde_json::from_str(&json).expect("observation should deserialize");
        assert_eq!(parsed.kind, RuntimeUsageObservationKind::FinalSummary);
        assert_eq!(parsed.usage.input_tokens, 3);
        assert_eq!(parsed.usage.output_tokens, 5);
        assert_eq!(parsed.usage.cache_creation_input_tokens, 7);
        assert_eq!(parsed.usage.cache_read_input_tokens, 11);
    }
}
