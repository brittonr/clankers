//! Prompt identity, model request, and assembly types for the host-facing runtime facade.

use std::collections::BTreeSet;

pub use clanker_message::AssembledPrompt;
pub use clanker_message::ContextReferenceKind;
pub use clanker_message::ContextReferenceRequest;
use clanker_message::Content;
pub use clanker_message::HostContext;
pub use clanker_message::PromptAssemblyPolicy;
pub use clanker_message::PromptProvenance;
pub use clanker_message::PromptSection;
pub use clanker_message::PromptSourceKind;
pub use clanker_message::PromptSourceRequest;
pub use clanker_message::PromptSources;
pub use clanker_message::SkillSnippet;
pub use clanker_message::UnsupportedContextReference;
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
        if !policy.allow_filesystem_discovery
            && (sources.filesystem_context_requested || !sources.filesystem_context.is_empty())
        {
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
        for entry in &sources.filesystem_context {
            let rendered = sanitize_prompt_context(&entry.content);
            sections.push(PromptSection {
                label: entry.label.clone(),
                content: rendered,
            });
            provenance.push(PromptProvenance {
                label: entry.label.clone(),
                source: PromptSourceKind::Filesystem,
                safe_summary: format!("filesystem:{}:{}chars", entry.label, entry.content.chars().count()),
            });
        }
        for skill in &sources.skill_snippets {
            let rendered = sanitize_prompt_context(&skill.content);
            sections.push(PromptSection {
                label: format!("skill:{}", skill.name),
                content: rendered,
            });
            provenance.push(PromptProvenance {
                label: sanitize_metadata_value(skill.name.clone()),
                source: PromptSourceKind::Skill,
                safe_summary: format!("skill:{}:{}chars", skill.source, skill.content.chars().count()),
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

pub trait PromptSourceService: Send + Sync + 'static {
    fn capability(&self) -> &'static str;
    fn resolve_sources(&self, request: PromptSourceRequest) -> Result<PromptSources, RuntimeError>;
}

#[derive(Debug, Clone)]
pub struct StaticPromptSourceService {
    sources: PromptSources,
    capability: &'static str,
}

impl StaticPromptSourceService {
    #[must_use]
    pub fn new(sources: PromptSources) -> Self {
        Self {
            sources,
            capability: "static_prompt_sources",
        }
    }

    #[must_use]
    pub fn with_capability(mut self, capability: &'static str) -> Self {
        self.capability = capability;
        self
    }
}

impl PromptSourceService for StaticPromptSourceService {
    fn capability(&self) -> &'static str {
        self.capability
    }

    fn resolve_sources(&self, request: PromptSourceRequest) -> Result<PromptSources, RuntimeError> {
        let _ = request;
        Ok(self.sources.clone())
    }
}

pub struct DisabledPromptSourceService;

impl PromptSourceService for DisabledPromptSourceService {
    fn capability(&self) -> &'static str {
        "disabled"
    }

    fn resolve_sources(&self, request: PromptSourceRequest) -> Result<PromptSources, RuntimeError> {
        let _ = request;
        Ok(PromptSources::default())
    }
}
