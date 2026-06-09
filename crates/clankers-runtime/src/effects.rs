//! Typed ability-style effect envelopes for host-owned runtime handlers.

use std::collections::BTreeMap;

pub use clanker_message::EffectAbilityClass;
pub use clanker_message::EffectCorrelationId;
pub use clanker_message::EffectResultStatus;
pub use clanker_message::REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION;
pub use clanker_message::RemoteArtifactEnvelope;
pub use clanker_message::RemoteDependencyFailure;
pub use clanker_message::RemoteDependencyFailureKind;
pub use clanker_message::RemoteExecutionArtifactKind;
pub use clanker_message::RemoteExecutionDependency;
pub use clanker_message::RemoteExecutionRequest;
pub use clanker_message::RemoteExecutionTarget;
pub use clanker_message::UcanAuthorizationMetadata;
use clankers_artifacts::ArtifactHash;
use clankers_artifacts::RedactionClass;
use serde::Deserialize;
use serde::Serialize;

use crate::events::contains_secret_marker;
use crate::events::sanitize_metadata_value;

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

/// Evaluate safe remote dependency sync without touching model/tool/provider resources.
#[must_use]
pub fn evaluate_remote_dependency_sync(
    request: &RemoteExecutionRequest,
    provided: &[RemoteArtifactEnvelope],
) -> RemoteDependencySyncReport {
    let provided_by_dependency = provided
        .iter()
        .map(|envelope| (remote_dependency_key(&envelope.dependency), envelope))
        .collect::<BTreeMap<_, _>>();
    let mut missing_safe_artifacts = Vec::new();
    let mut failures = Vec::new();

    for dependency in &request.required_artifacts {
        let Some(envelope) = provided_by_dependency.get(&remote_dependency_key(dependency)) else {
            missing_safe_artifacts.push(dependency.clone());
            failures.push(RemoteDependencyFailure::new(
                dependency.clone(),
                RemoteDependencyFailureKind::MissingSafeArtifact,
                "safe artifact missing",
            ));
            continue;
        };
        if envelope.redaction_class == RedactionClass::Secret {
            failures.push(RemoteDependencyFailure::new(
                dependency.clone(),
                RemoteDependencyFailureKind::SecretDependencyDenied,
                "secret dependency denied",
            ));
        } else if envelope.schema_version != REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION {
            failures.push(RemoteDependencyFailure::new(
                dependency.clone(),
                RemoteDependencyFailureKind::UnsupportedVersion,
                "unsupported artifact schema version",
            ));
        } else if envelope.computed_hash != dependency.hash {
            failures.push(RemoteDependencyFailure::new(
                dependency.clone(),
                RemoteDependencyFailureKind::HashMismatch,
                "artifact hash mismatch",
            ));
        }
    }

    RemoteDependencySyncReport {
        missing_safe_artifacts,
        failures,
    }
}

fn remote_dependency_key(dependency: &RemoteExecutionDependency) -> (RemoteExecutionArtifactKind, String) {
    (dependency.kind, dependency.hash.hex())
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

/// Result of fail-closed effect gating around a host side effect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EffectGate<T> {
    /// The handler explicitly allowed the effect and the host operation ran.
    Executed { value: T, receipt: EffectResult },
    /// The effect was denied, unavailable, simulated, or replayed before the operation ran.
    Blocked { receipt: EffectResult },
}

impl<T> EffectGate<T> {
    /// Borrow the safe handler receipt regardless of whether the operation ran.
    #[must_use]
    pub fn receipt(&self) -> &EffectResult {
        match self {
            Self::Executed { receipt, .. } | Self::Blocked { receipt } => receipt,
        }
    }

    /// Return true only when the real operation executed.
    #[must_use]
    pub fn executed(&self) -> bool {
        matches!(self, Self::Executed { .. })
    }
}

/// Gate a host side-effect behind an explicit effect handler.
///
/// Missing handlers, class mismatches, denied requests, simulated results, and replay-only receipts
/// all fail closed without invoking `operation`. Only an explicit `Allowed` result executes the
/// closure.
pub fn run_effect_fail_closed<T>(
    request: &EffectRequest,
    handler: Option<&dyn EffectHandler>,
    operation: impl FnOnce() -> T,
) -> EffectGate<T> {
    let Some(handler) = handler else {
        return EffectGate::Blocked {
            receipt: EffectResult::new(request, EffectResultStatus::Unavailable, "missing effect handler"),
        };
    };
    let receipt = if handler.class() == request.class {
        handler.handle(request)
    } else {
        EffectResult::new(request, EffectResultStatus::Unavailable, "handler class mismatch")
    };
    if receipt.status == EffectResultStatus::Allowed {
        EffectGate::Executed {
            value: operation(),
            receipt,
        }
    } else {
        EffectGate::Blocked { receipt }
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
    fn effect_receipt_carries_redacted_ucan_authorization_metadata() {
        let request = EffectRequest::new(
            EffectAbilityClass::Filesystem,
            EffectCorrelationId::from_static("ucan-receipt"),
            RedactionClass::MetadataOnly,
        );
        let compact_token = format!("ey{}.ey{}.sig", "a".repeat(40), "b".repeat(40));
        let metadata = UcanAuthorizationMetadata::new("file/read", "clankers:file:///workspace/src/lib.rs", "allowed")
            .with_issuer("did:key:z6Missuer")
            .with_audience("did:key:z6Maudience")
            .with_proof_references(["proof-b", "proof-a", "proof-a", compact_token.as_str()])
            .with_caveat_ids(["path-prefix", "max-bytes"])
            .with_replay_status("checked")
            .with_revocation_status("clear");

        let receipt =
            EffectResult::new(&request, EffectResultStatus::Allowed, "read ok").with_ucan_authorization(metadata);
        let serialized = serde_json::to_string(&receipt).expect("receipt json");

        assert!(serialized.contains("file/read"));
        assert!(serialized.contains("proof-a"));
        assert!(serialized.contains("proof-b"));
        assert!(!serialized.contains(compact_token.as_str()));
        assert!(serialized.contains("redacted"));
    }

    #[test]
    fn denied_ucan_receipt_records_class_without_secret_details() {
        let request = EffectRequest::new(
            EffectAbilityClass::Secret,
            EffectCorrelationId::from_static("ucan-denial"),
            RedactionClass::MetadataOnly,
        );
        let metadata = UcanAuthorizationMetadata::new("secret/read", "secret://api", "denied")
            .with_denial_class("revoked: Bearer secret-token")
            .with_revocation_status("revoked");

        let receipt = EffectResult::new(&request, EffectResultStatus::Denied, "Bearer secret-token denied")
            .with_ucan_authorization(metadata);
        let serialized = serde_json::to_string(&receipt).expect("denial json");

        assert!(serialized.contains("denied"));
        assert!(serialized.contains("revoked"));
        assert!(!serialized.contains("Bearer"));
        assert!(!serialized.contains("secret-token"));
    }

    #[test]
    fn artifact_envelope_carries_safe_ucan_metadata_only() {
        let dependency =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, ArtifactHash::digest(b"policy"));
        let metadata = UcanAuthorizationMetadata::new("artifact/read", "artifact://policy", "allowed")
            .with_proof_references(["proof-ref"]);
        let envelope = RemoteArtifactEnvelope::new(
            dependency,
            REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION,
            ArtifactHash::digest(b"policy"),
            RedactionClass::Public,
        )
        .with_ucan_authorization(metadata);
        let serialized = serde_json::to_string(&envelope).expect("envelope json");

        assert!(serialized.contains("artifact/read"));
        assert!(serialized.contains("proof-ref"));
        assert!(!serialized.contains("compact-token"));
    }

    #[test]
    fn remote_execution_request_declares_safe_dependencies_by_artifact_hash() {
        let prompt = ArtifactHash::digest(b"prompt");
        let skill = ArtifactHash::digest(b"skill");
        let tool_schema = ArtifactHash::digest(b"schema");
        let manifest = ArtifactHash::digest(b"manifest");
        let policy = ArtifactHash::digest(b"policy");
        let request = RemoteExecutionRequest::new(
            RemoteExecutionTarget::RemoteDaemon,
            EffectCorrelationId::from_static("remote-1"),
        )
        .with_required_artifacts([
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, policy),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, prompt),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Skill, skill),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::ToolSchema, tool_schema),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Manifest, manifest),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, policy),
        ]);

        assert_eq!(request.target, RemoteExecutionTarget::RemoteDaemon);
        assert_eq!(request.required_artifacts.len(), 5);
        assert_eq!(request.required_artifacts, vec![
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, prompt),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Skill, skill),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::ToolSchema, tool_schema),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Manifest, manifest),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, policy),
        ]);
        let serialized = serde_json::to_string(&request).expect("remote request json");
        assert!(serialized.contains("remote-daemon"));
        assert!(serialized.contains("tool-schema"));
        assert!(!serialized.contains("Bearer"));
        assert!(!serialized.contains("token"));
    }

    #[test]
    fn remote_execution_required_hashes_project_for_effect_dependencies() {
        let prompt = ArtifactHash::digest(b"prompt");
        let policy = ArtifactHash::digest(b"policy");
        let request = RemoteExecutionRequest::new(
            RemoteExecutionTarget::Subagent,
            EffectCorrelationId::from_static("subagent-1"),
        )
        .with_required_artifacts([
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, prompt),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, policy),
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Manifest, prompt),
        ]);

        let mut expected = vec![prompt, policy];
        expected.sort_by_key(|hash| hash.hex());
        assert_eq!(request.required_hashes(), expected);
    }

    #[test]
    fn remote_dependency_sync_reports_missing_safe_artifacts_by_hash() {
        let prompt =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, ArtifactHash::digest(b"prompt"));
        let request = RemoteExecutionRequest::new(
            RemoteExecutionTarget::RemoteDaemon,
            EffectCorrelationId::from_static("missing-remote"),
        )
        .with_required_artifacts([prompt.clone()]);
        let report = evaluate_remote_dependency_sync(&request, &[]);

        assert!(!report.ready());
        assert_eq!(report.missing_safe_artifacts, vec![prompt.clone()]);
        assert_eq!(report.failures[0].kind, RemoteDependencyFailureKind::MissingSafeArtifact);
        assert_eq!(report.failures[0].dependency, prompt);
    }

    #[test]
    fn remote_dependency_sync_fails_on_hash_mismatch_unsupported_version_and_secret_dependencies() {
        let prompt =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, ArtifactHash::digest(b"prompt"));
        let manifest =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Manifest, ArtifactHash::digest(b"manifest"));
        let policy =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Policy, ArtifactHash::digest(b"policy"));
        let request = RemoteExecutionRequest::new(
            RemoteExecutionTarget::RemoteDaemon,
            EffectCorrelationId::from_static("bad-sync"),
        )
        .with_required_artifacts([prompt.clone(), manifest.clone(), policy.clone()]);
        let report = evaluate_remote_dependency_sync(&request, &[
            RemoteArtifactEnvelope::new(
                prompt.clone(),
                REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION,
                ArtifactHash::digest(b"different prompt"),
                RedactionClass::Public,
            ),
            RemoteArtifactEnvelope::new(
                manifest.clone(),
                REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION + 1,
                manifest.hash,
                RedactionClass::Public,
            ),
            RemoteArtifactEnvelope::new(
                policy.clone(),
                REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION,
                policy.hash,
                RedactionClass::Secret,
            ),
        ]);

        assert!(!report.ready());
        let kinds = report.failures.iter().map(|failure| failure.kind).collect::<Vec<_>>();
        assert_eq!(kinds, vec![
            RemoteDependencyFailureKind::HashMismatch,
            RemoteDependencyFailureKind::UnsupportedVersion,
            RemoteDependencyFailureKind::SecretDependencyDenied,
        ]);
        let serialized = serde_json::to_string(&report).expect("sync report json");
        assert!(!serialized.contains("Bearer"));
        assert!(!serialized.contains("secret-token"));
    }

    #[test]
    fn remote_dependency_sync_ready_report_converts_to_allowed_effect_result() {
        let prompt =
            RemoteExecutionDependency::new(RemoteExecutionArtifactKind::Prompt, ArtifactHash::digest(b"prompt"));
        let remote = RemoteExecutionRequest::new(
            RemoteExecutionTarget::Subagent,
            EffectCorrelationId::from_static("ready-sync"),
        )
        .with_required_artifacts([prompt.clone()]);
        let report = evaluate_remote_dependency_sync(&remote, &[RemoteArtifactEnvelope::new(
            prompt.clone(),
            REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION,
            prompt.hash,
            RedactionClass::Public,
        )]);
        let effect_request = EffectRequest::new(
            EffectAbilityClass::Provider,
            EffectCorrelationId::from_static("model-after-sync"),
            RedactionClass::MetadataOnly,
        )
        .with_artifact_dependencies(remote.required_hashes());

        assert!(report.ready());
        assert_eq!(report.to_effect_result(&effect_request).status, EffectResultStatus::Allowed);
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

    #[test]
    fn fail_closed_sentinel_blocks_absent_handlers_before_side_effects() {
        for class in [
            EffectAbilityClass::Filesystem,
            EffectAbilityClass::Shell,
            EffectAbilityClass::Network,
            EffectAbilityClass::Browser,
            EffectAbilityClass::Provider,
            EffectAbilityClass::Secret,
        ] {
            let request = EffectRequest::new(
                class,
                EffectCorrelationId::from_static("absent-handler"),
                RedactionClass::MetadataOnly,
            );
            let mut invoked = false;
            let gate = run_effect_fail_closed(&request, None, || {
                invoked = true;
                "side effect ran"
            });

            assert!(!invoked, "{class:?} operation must not run without a handler");
            assert!(!gate.executed());
            assert_eq!(gate.receipt().status, EffectResultStatus::Unavailable);
        }
    }

    #[test]
    fn fail_closed_sentinel_blocks_denied_handlers_before_side_effects() {
        let handler = StaticEffectHandler::new(EffectAbilityClass::Shell, EffectHandlerMode::Deny {
            reason: "process execution disabled".to_owned(),
        });
        let request = EffectRequest::new(
            EffectAbilityClass::Shell,
            EffectCorrelationId::from_static("denied-process"),
            RedactionClass::MetadataOnly,
        );
        let mut invoked = false;
        let gate = run_effect_fail_closed(&request, Some(&handler), || {
            invoked = true;
            7
        });

        assert!(!invoked);
        assert!(!gate.executed());
        assert_eq!(gate.receipt().status, EffectResultStatus::Denied);
        assert_eq!(gate.receipt().safe_summary, "process execution disabled");
    }

    #[test]
    fn fail_closed_sentinel_only_executes_after_explicit_allow() {
        let handler = StaticEffectHandler::new(EffectAbilityClass::Network, EffectHandlerMode::Allow);
        let request = EffectRequest::new(
            EffectAbilityClass::Network,
            EffectCorrelationId::from_static("allowed-socket"),
            RedactionClass::MetadataOnly,
        );
        let mut invoked = false;
        let gate = run_effect_fail_closed(&request, Some(&handler), || {
            invoked = true;
            "connected"
        });

        assert!(invoked);
        match gate {
            EffectGate::Executed { value, receipt } => {
                assert_eq!(value, "connected");
                assert_eq!(receipt.status, EffectResultStatus::Allowed);
            }
            EffectGate::Blocked { .. } => panic!("allowed effect should execute"),
        }
    }

    #[test]
    fn fail_closed_sentinel_treats_simulate_replay_and_mismatch_as_non_executing_receipts() {
        let request = EffectRequest::new(
            EffectAbilityClass::Provider,
            EffectCorrelationId::from_static("provider-call"),
            RedactionClass::MetadataOnly,
        );
        let simulate = StaticEffectHandler::new(EffectAbilityClass::Provider, EffectHandlerMode::Simulate {
            summary: "model response simulated".to_owned(),
        });
        let mismatch = StaticEffectHandler::new(EffectAbilityClass::Browser, EffectHandlerMode::Allow);

        for (handler, expected_status) in [
            (&simulate as &dyn EffectHandler, EffectResultStatus::Simulated),
            (&mismatch as &dyn EffectHandler, EffectResultStatus::Unavailable),
        ] {
            let mut invoked = false;
            let gate = run_effect_fail_closed(&request, Some(handler), || {
                invoked = true;
                "side effect ran"
            });
            assert!(!invoked);
            assert!(!gate.executed());
            assert_eq!(gate.receipt().status, expected_status);
        }
    }
}
