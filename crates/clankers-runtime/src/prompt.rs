//! Prompt identity, model request, and assembly types for the host-facing runtime facade.

use std::collections::BTreeSet;

use clanker_message::Content;
use clanker_message::Usage;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::EventMetadata;
use crate::RuntimeError;
use crate::SessionEvent;
use crate::SessionId;
use crate::SessionLedgerMessage;
use crate::events::contains_secret_marker;
use crate::events::sanitize_metadata_value;

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
    #[serde(default)]
    pub history: Vec<SessionLedgerMessage>,
    #[serde(default)]
    pub metadata: ModelRequestMetadata,
}

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

/// Semantic events returned by a model adapter.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelResponse {
    pub events: Vec<SessionEvent>,
    #[serde(default)]
    pub engine_content: Vec<Content>,
    #[serde(default)]
    pub usage: Option<Usage>,
    #[serde(default)]
    pub stop_reason: Option<clanker_message::StopReason>,
    #[serde(default)]
    pub failure: Option<ModelFailure>,
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
            engine_content: Vec::new(),
            usage: None,
            stop_reason: None,
            failure: None,
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
