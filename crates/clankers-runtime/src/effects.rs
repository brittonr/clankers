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

/// Safe content-addressed artifact kinds that remote/subagent execution can declare.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteExecutionArtifactKind {
    /// System/developer/user prompt material safe to sync by hash.
    Prompt,
    /// Skill instructions or support files safe to sync by hash.
    Skill,
    /// Tool schema or descriptor material safe to sync by hash.
    ToolSchema,
    /// Plugin/tool/extension manifest material safe to sync by hash.
    Manifest,
    /// Non-secret policy metadata safe to sync by hash.
    Policy,
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

/// Remote execution target shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteExecutionTarget {
    /// In-process or subprocess subagent.
    Subagent,
    /// Remote daemon peer.
    RemoteDaemon,
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
            ability: sanitize_authorization_value(ability.into()),
            resource_uri: sanitize_authorization_value(resource_uri.into()),
            status: sanitize_authorization_value(status.into()),
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
        self.issuer = Some(sanitize_authorization_value(issuer.into()));
        self
    }

    #[must_use]
    pub fn with_audience(mut self, audience: impl Into<String>) -> Self {
        self.audience = Some(sanitize_authorization_value(audience.into()));
        self
    }

    #[must_use]
    pub fn with_proof_references<I, S>(mut self, references: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.proof_references = sorted_sanitized_values(references);
        self
    }

    #[must_use]
    pub fn with_caveat_ids<I, S>(mut self, caveat_ids: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.caveat_ids = sorted_sanitized_values(caveat_ids);
        self
    }

    #[must_use]
    pub fn with_replay_status(mut self, replay_status: impl Into<String>) -> Self {
        self.replay_status = Some(sanitize_authorization_value(replay_status.into()));
        self
    }

    #[must_use]
    pub fn with_revocation_status(mut self, revocation_status: impl Into<String>) -> Self {
        self.revocation_status = Some(sanitize_authorization_value(revocation_status.into()));
        self
    }

    #[must_use]
    pub fn with_denial_class(mut self, denial_class: impl Into<String>) -> Self {
        self.denial_class = Some(sanitize_authorization_value(denial_class.into()));
        self
    }
}

fn sorted_sanitized_values<I, S>(values: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut sanitized = values.into_iter().map(|value| sanitize_authorization_value(value.into())).collect::<Vec<_>>();
    sanitized.sort();
    sanitized.dedup();
    sanitized
}

fn sanitize_authorization_value(value: String) -> String {
    if contains_secret_marker(&value) || looks_like_compact_token(&value) {
        "[redacted-secret-marker]".to_owned()
    } else {
        sanitize_metadata_value(value)
    }
}

fn looks_like_compact_token(value: &str) -> bool {
    value.matches('.').count() == 2 && value.starts_with("ey") && value.len() > 80
}

/// Fail-closed remote dependency sync failure kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RemoteDependencyFailureKind {
    /// A safe requested artifact is absent and should be requested by hash.
    MissingSafeArtifact,
    /// The peer returned an envelope version this runtime does not support.
    UnsupportedVersion,
    /// The returned envelope canonical hash did not match the requested hash.
    HashMismatch,
    /// The dependency would require secret material that must not be synced.
    SecretDependencyDenied,
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
    fn new(
        dependency: RemoteExecutionDependency,
        kind: RemoteDependencyFailureKind,
        safe_summary: impl Into<String>,
    ) -> Self {
        Self {
            dependency,
            kind,
            safe_summary: sanitize_metadata_value(safe_summary.into()),
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
