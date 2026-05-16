//! Typed ability-style effect envelopes for host-owned runtime handlers.

use std::collections::BTreeMap;

use clankers_artifacts::ArtifactHash;
use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::events::contains_secret_marker;
use crate::events::sanitize_metadata_value;

/// Effectful capability class requested by runtime/tool code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectAbilityClass {
    /// Host-selected filesystem reads or writes.
    Filesystem,
    /// Shell command execution.
    Shell,
    /// Network socket or HTTP access.
    Network,
    /// Credential, token, or secret lookup.
    Secret,
    /// Browser automation or browser state access.
    Browser,
    /// Scheduler, timer, or delayed execution.
    Scheduler,
    /// Model/provider API access.
    Provider,
    /// Plugin or extension runtime execution.
    Plugin,
    /// Built-in or external tool invocation.
    Tool,
    /// User-visible delivery channel such as Matrix, ACP, or TTS.
    Delivery,
}

/// Stable correlation identifier carried through requests, results, and receipts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EffectCorrelationId(String);

impl EffectCorrelationId {
    /// Mint a new opaque correlation ID.
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
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

impl Default for EffectCorrelationId {
    fn default() -> Self {
        Self::new()
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
        let key = sanitize_metadata_value(key.into());
        let raw_value = value.into();
        let value = if contains_secret_marker(&raw_value) {
            "[redacted-secret-marker]".to_owned()
        } else {
            sanitize_metadata_value(raw_value)
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

/// Handler outcome kind for a typed effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectResultStatus {
    /// Real side effect was allowed by host policy.
    Allowed,
    /// Request was denied before side effects.
    Denied,
    /// Handler returned a simulated result without touching the resource.
    Simulated,
    /// Handler returned a recorded replay result.
    Replayed,
    /// Required handler or dependency was absent.
    Unavailable,
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
}

impl EffectResult {
    /// Build a redacted result envelope for a request.
    #[must_use]
    pub fn new(request: &EffectRequest, status: EffectResultStatus, safe_summary: impl Into<String>) -> Self {
        let raw_summary = safe_summary.into();
        let safe_summary = if contains_secret_marker(&raw_summary) {
            "[redacted-secret-marker]".to_owned()
        } else {
            sanitize_metadata_value(raw_summary)
        };
        Self {
            request: EffectRequestRef::from(request),
            status,
            output_artifact: None,
            safe_summary,
        }
    }

    /// Attach a result artifact hash.
    #[must_use]
    pub fn with_output_artifact(mut self, hash: ArtifactHash) -> Self {
        self.output_artifact = Some(hash);
        self
    }
}

/// Host handler behavior mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectHandlerMode {
    /// Permit the host shell to execute the real operation.
    Allow,
    /// Deny before side effects occur.
    Deny { reason: String },
    /// Return a simulation receipt without touching the resource.
    Simulate { summary: String },
    /// Return a previously recorded result by correlation ID.
    Replay {
        receipts: BTreeMap<EffectCorrelationId, EffectResult>,
    },
}

/// Host-owned policy handler for one effect class.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticEffectHandler {
    class: EffectAbilityClass,
    mode: EffectHandlerMode,
}

impl StaticEffectHandler {
    /// Create a host-owned handler for an effect class.
    #[must_use]
    pub fn new(class: EffectAbilityClass, mode: EffectHandlerMode) -> Self {
        Self { class, mode }
    }

    /// Create deny-by-default handlers for the initial file/shell/network/secret/tool subset.
    #[must_use]
    pub fn initial_subset_deny_by_default(reason: impl Into<String>) -> Vec<Self> {
        let reason = reason.into();
        initial_effect_handler_subset()
            .into_iter()
            .map(|class| Self::new(class, EffectHandlerMode::Deny { reason: reason.clone() }))
            .collect()
    }
}

impl EffectHandler for StaticEffectHandler {
    fn class(&self) -> EffectAbilityClass {
        self.class
    }

    fn handle(&self, request: &EffectRequest) -> EffectResult {
        if request.class != self.class {
            return EffectResult::new(request, EffectResultStatus::Unavailable, "handler class mismatch");
        }
        match &self.mode {
            EffectHandlerMode::Allow => {
                EffectResult::new(request, EffectResultStatus::Allowed, "allowed by host handler")
            }
            EffectHandlerMode::Deny { reason } => {
                EffectResult::new(request, EffectResultStatus::Denied, reason.clone())
            }
            EffectHandlerMode::Simulate { summary } => {
                EffectResult::new(request, EffectResultStatus::Simulated, summary.clone())
            }
            EffectHandlerMode::Replay { receipts } => {
                receipts.get(&request.correlation_id).cloned().unwrap_or_else(|| {
                    EffectResult::new(request, EffectResultStatus::Unavailable, "missing replay receipt")
                })
            }
        }
    }
}

/// Initial classes supported by the first host-owned handler matrix.
#[must_use]
pub fn initial_effect_handler_subset() -> Vec<EffectAbilityClass> {
    vec![
        EffectAbilityClass::Filesystem,
        EffectAbilityClass::Shell,
        EffectAbilityClass::Network,
        EffectAbilityClass::Secret,
        EffectAbilityClass::Tool,
    ]
}

/// Host-owned handler boundary for typed effects.
pub trait EffectHandler: Send + Sync {
    /// Effect class this handler accepts.
    fn class(&self) -> EffectAbilityClass;

    /// Handle a request, returning an explicit allow/deny/simulate/replay/unavailable result.
    fn handle(&self, request: &EffectRequest) -> EffectResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effect_request_carries_policy_metadata_and_hash_dependencies() {
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

        assert_eq!(request.class, EffectAbilityClass::Shell);
        assert_eq!(request.correlation_id.as_str(), "effect-1");
        assert_eq!(request.input_schema_hash, Some(schema_hash));
        let mut expected_dependencies = vec![dep_a, dep_b];
        expected_dependencies.sort_by_key(|hash| hash.hex());
        assert_eq!(request.declared_artifact_dependencies, expected_dependencies);
        assert_eq!(request.safe_source_metadata.get("tool"), Some(&"bash".to_owned()));
        let metadata_json = serde_json::to_string(&request.safe_source_metadata).expect("metadata json");
        assert!(!metadata_json.contains("Bearer"));
        assert!(!metadata_json.contains("secret-token"));
        assert!(metadata_json.contains("redacted"));
    }

    #[test]
    fn effect_result_redacts_secret_markers_and_preserves_request_ref() {
        let request = EffectRequest::new(
            EffectAbilityClass::Secret,
            EffectCorrelationId::from_static("secret-lookup"),
            RedactionClass::Secret,
        );
        let result = EffectResult::new(&request, EffectResultStatus::Denied, "token secret value denied");

        assert_eq!(result.request.class, EffectAbilityClass::Secret);
        assert_eq!(result.request.correlation_id.as_str(), "secret-lookup");
        assert_eq!(result.status, EffectResultStatus::Denied);
        assert_eq!(result.safe_summary, "[redacted-secret-marker]");
    }

    struct DenyShell;

    impl EffectHandler for DenyShell {
        fn class(&self) -> EffectAbilityClass {
            EffectAbilityClass::Shell
        }

        fn handle(&self, request: &EffectRequest) -> EffectResult {
            EffectResult::new(request, EffectResultStatus::Denied, "shell disabled by policy")
        }
    }

    #[test]
    fn effect_handler_trait_boundary_returns_explicit_status() {
        let request = EffectRequest::new(
            EffectAbilityClass::Shell,
            EffectCorrelationId::from_static("shell-1"),
            RedactionClass::MetadataOnly,
        );
        let handler = DenyShell;
        let result = handler.handle(&request);

        assert_eq!(handler.class(), EffectAbilityClass::Shell);
        assert_eq!(result.status, EffectResultStatus::Denied);
        assert_eq!(result.request.correlation_id, request.correlation_id);
    }

    #[test]
    fn static_effect_handlers_cover_allow_deny_simulate_and_replay_modes() {
        let request = EffectRequest::new(
            EffectAbilityClass::Filesystem,
            EffectCorrelationId::from_static("fs-1"),
            RedactionClass::MetadataOnly,
        );
        let allow = StaticEffectHandler::new(EffectAbilityClass::Filesystem, EffectHandlerMode::Allow);
        let deny = StaticEffectHandler::new(EffectAbilityClass::Filesystem, EffectHandlerMode::Deny {
            reason: "disabled".to_owned(),
        });
        let simulate = StaticEffectHandler::new(EffectAbilityClass::Filesystem, EffectHandlerMode::Simulate {
            summary: "simulated".to_owned(),
        });
        let replayed = EffectResult::new(&request, EffectResultStatus::Replayed, "from receipt");
        let replay = StaticEffectHandler::new(EffectAbilityClass::Filesystem, EffectHandlerMode::Replay {
            receipts: BTreeMap::from([(request.correlation_id.clone(), replayed.clone())]),
        });

        assert_eq!(allow.handle(&request).status, EffectResultStatus::Allowed);
        assert_eq!(deny.handle(&request).status, EffectResultStatus::Denied);
        assert_eq!(simulate.handle(&request).status, EffectResultStatus::Simulated);
        assert_eq!(replay.handle(&request), replayed);
    }

    #[test]
    fn initial_handler_subset_is_deny_by_default_for_effectful_core_classes() {
        let handlers = StaticEffectHandler::initial_subset_deny_by_default("not enabled");
        let classes = handlers.iter().map(EffectHandler::class).collect::<Vec<_>>();
        assert_eq!(classes, initial_effect_handler_subset());

        for handler in handlers {
            let request = EffectRequest::new(
                handler.class(),
                EffectCorrelationId::from_static("deny-default"),
                RedactionClass::MetadataOnly,
            );
            assert_eq!(handler.handle(&request).status, EffectResultStatus::Denied);
        }
    }
}
