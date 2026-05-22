//! Typed Steel self-mutation request DTOs and pure authorization core.
//!
//! Steel scripts never receive filesystem, process, git, network, provider,
//! credential, daemon, TUI, or native-tool authority through this module. They
//! can only describe an intended mutation. The Rust host evaluates the exported
//! Nickel policy plus safe UCAN metadata before any shell code may write bytes.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use clankers_artifacts::ArtifactHash;
use serde::Deserialize;
use serde::Serialize;
use thiserror::Error;

pub const STEEL_MUTATION_POLICY_SCHEMA: &str = "clankers.steel_self_mutation.policy.v1";
pub const STEEL_MUTATION_RECEIPT_SCHEMA: &str = "clankers.steel_self_mutation.receipt.v1";
pub const STEEL_MUTATION_DECISION_SCHEMA: &str = "clankers.steel_self_mutation.decision.v1";
pub const STEEL_MUTATION_PREFLIGHT_SCHEMA: &str = "clankers.steel_self_mutation.preflight.v1";
const STEEL_MUTATION_SESSION_CAPABILITY: &str = "steel-self-mutation";
const WORKSPACE_MUTATION_SESSION_CAPABILITY: &str = "workspace-mutation";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationPolicy {
    pub schema: String,
    pub target_classes: Vec<SteelMutationTargetClass>,
    pub mutation_verbs: Vec<SteelMutationVerbPolicy>,
    pub runtime_profiles: Vec<SteelMutationRuntimeProfile>,
    pub ucan: SteelMutationUcanPolicy,
    pub receipt: SteelMutationReceiptPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationTargetClass {
    pub name: String,
    pub resource_prefix: String,
    pub allowed_path_roots: Vec<String>,
    pub denied_path_patterns: Vec<String>,
    pub allowed_verbs: Vec<String>,
    pub approval_tier: String,
    pub preflight_profile: String,
    pub verification_profile: String,
    pub rollback_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationVerbPolicy {
    pub name: String,
    pub host_function: String,
    pub ucan_ability: String,
    pub requires_approval: bool,
    pub writes_bytes: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationRuntimeProfile {
    pub name: String,
    pub steel_profile: String,
    pub max_source_bytes: u64,
    pub max_output_bytes: u64,
    pub max_host_calls: u64,
    pub ambient_authority: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationUcanPolicy {
    pub required: bool,
    pub audience_binding: String,
    pub deny_wildcard_resources: bool,
    pub max_delegation_depth: u32,
    pub safe_receipt_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationReceiptPolicy {
    pub schema: String,
    pub include_policy_hash: bool,
    pub include_safe_ucan_metadata: bool,
    pub redact_fields: Vec<String>,
    pub forbidden_receipt_markers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationRequest {
    pub target_class: String,
    pub verb: String,
    pub resource: String,
    pub expected_audience: String,
    pub relative_path: String,
    pub intent: String,
    pub patch: Option<SteelMutationPatch>,
    pub approval: SteelMutationApproval,
    pub ucan: Option<SteelMutationUcanGrant>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationPatch {
    pub format: SteelMutationPatchFormat,
    pub bytes: u64,
    pub body_blake3: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationPatchFormat {
    UnifiedDiff,
    FullReplace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationApproval {
    pub approved: bool,
    pub tier: String,
    pub reviewer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationUcanGrant {
    pub ability: String,
    pub resource: String,
    pub audience: String,
    pub expiry_status: SteelMutationUcanExpiryStatus,
    pub delegation_depth: u32,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationUcanExpiryStatus {
    Valid,
    Expired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationDecision {
    pub schema: String,
    pub outcome: SteelMutationDecisionOutcome,
    pub reason_code: SteelMutationReasonCode,
    pub safe_message: String,
    pub host_function: Option<String>,
    pub target_class: String,
    pub normalized_path: Option<String>,
    pub required_ucan_ability: Option<String>,
    pub required_ucan_resource: Option<String>,
    pub safe_ucan_metadata: Option<SteelMutationSafeUcanMetadata>,
    pub preflight_profile: Option<String>,
    pub verification_profile: Option<String>,
    pub rollback_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationDecisionOutcome {
    Allowed,
    Denied,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationSafeUcanMetadata {
    pub ability: String,
    pub resource: String,
    pub audience: String,
    pub expiry_status: SteelMutationUcanExpiryStatus,
    pub delegation_depth: u32,
    pub authorization_outcome: SteelMutationDecisionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationHostContext {
    pub policy_hash: ArtifactHash,
    pub session_capabilities: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub target_hash: Option<ArtifactHash>,
    pub repository_dirty: bool,
    pub checkpoint_id: Option<String>,
}

impl SteelMutationHostContext {
    #[must_use]
    pub fn new(policy_hash: ArtifactHash) -> Self {
        Self {
            policy_hash,
            session_capabilities: Vec::new(),
            disabled_tools: Vec::new(),
            target_hash: None,
            repository_dirty: false,
            checkpoint_id: None,
        }
    }

    #[must_use]
    pub fn with_session_capabilities<I, S>(mut self, capabilities: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.session_capabilities = capabilities.into_iter().map(Into::into).collect();
        self.session_capabilities.sort();
        self.session_capabilities.dedup();
        self
    }

    #[must_use]
    pub fn with_disabled_tools<I, S>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.disabled_tools = tools.into_iter().map(Into::into).collect();
        self.disabled_tools.sort();
        self.disabled_tools.dedup();
        self
    }

    #[must_use]
    pub fn with_target_hash(mut self, hash: ArtifactHash) -> Self {
        self.target_hash = Some(hash);
        self
    }

    #[must_use]
    pub fn with_dirty_repository(mut self, checkpoint_id: Option<String>) -> Self {
        self.repository_dirty = true;
        self.checkpoint_id = checkpoint_id;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationHostPreflightReceipt {
    pub schema: String,
    pub status: SteelMutationHostPreflightStatus,
    pub reason_code: SteelMutationHostPreflightReason,
    pub safe_message: String,
    pub decision: SteelMutationDecision,
    pub host_function: Option<String>,
    pub normalized_path: Option<String>,
    pub policy_hash: ArtifactHash,
    pub target_hash: Option<ArtifactHash>,
    pub checkpoint: SteelMutationCheckpointPlan,
    pub verification_profile: Option<String>,
    pub safe_ucan_metadata: Option<SteelMutationSafeUcanMetadata>,
    pub writes_performed: bool,
}

impl SteelMutationHostPreflightReceipt {
    #[must_use]
    pub fn receipt_hash(&self) -> ArtifactHash {
        let bytes = serde_json::to_vec(self).expect("Steel mutation preflight receipt serializes");
        ArtifactHash::digest(&bytes)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SteelMutationHostPreflightStatus {
    Ready,
    Denied,
    Blocked,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SteelMutationCheckpointPlan {
    pub required: bool,
    pub checkpoint_id: Option<String>,
    pub target_hash: Option<ArtifactHash>,
    pub policy_hash: ArtifactHash,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SteelMutationPolicyParseError {
    #[error("failed to parse Steel mutation policy: {message}")]
    Json { message: String },
}

pub fn parse_steel_mutation_policy(text: &str) -> Result<SteelMutationPolicy, SteelMutationPolicyParseError> {
    serde_json::from_str(text).map_err(|error| SteelMutationPolicyParseError::Json {
        message: error.to_string(),
    })
}

#[must_use]
pub fn preflight_steel_mutation_host_function(
    policy: &SteelMutationPolicy,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> SteelMutationHostPreflightReceipt {
    let decision = authorize_steel_mutation(policy, request);
    let checkpoint = checkpoint_plan(&decision, context);
    if decision.outcome == SteelMutationDecisionOutcome::Denied {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Denied,
            SteelMutationHostPreflightReason::DecisionDenied,
            "Rust host denied mutation before preflight planning",
        );
    }
    if let Some(missing) = missing_required_capability(&decision, request, context) {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::MissingSessionCapability,
            format!("session lacks required mutation capability `{missing}`"),
        );
    }
    if host_function_is_disabled(&decision, request, context) {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::DisabledHostFunction,
            "requested Steel mutation host function is disabled for this session",
        );
    }
    if context.repository_dirty && context.checkpoint_id.is_none() {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::DirtyRepositoryNeedsCheckpoint,
            "dirty repository requires an explicit checkpoint before mutation",
        );
    }
    if decision.rollback_required && context.target_hash.is_none() {
        return host_preflight_receipt(
            decision,
            context,
            checkpoint,
            SteelMutationHostPreflightStatus::Blocked,
            SteelMutationHostPreflightReason::MissingTargetHash,
            "rollback-required mutation must capture a target hash before writing",
        );
    }
    host_preflight_receipt(
        decision,
        context,
        checkpoint,
        SteelMutationHostPreflightStatus::Ready,
        SteelMutationHostPreflightReason::Ready,
        "Steel mutation host function is ready for imperative shell execution",
    )
}

#[must_use]
pub fn authorize_steel_mutation(policy: &SteelMutationPolicy, request: &SteelMutationRequest) -> SteelMutationDecision {
    let Some(target) = target_class(policy, &request.target_class) else {
        return deny(
            request,
            SteelMutationReasonCode::UnknownTargetClass,
            "mutation target class is not declared by policy",
            None,
            None,
            None,
        );
    };
    let Some(verb) = verb_policy(policy, &request.verb) else {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::UnknownVerb,
            "mutation verb is not declared by policy",
            None,
            None,
        );
    };
    if !policy_is_safe(policy) {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::InvalidPolicy,
            "mutation policy is not fail-closed",
            Some(verb),
            None,
        );
    }
    if !target.allowed_verbs.iter().any(|allowed| allowed == &request.verb) {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::VerbNotAllowedForTarget,
            "mutation verb is not allowed for target class",
            Some(verb),
            None,
        );
    }
    let normalized_path = match normalize_relative_path(&request.relative_path) {
        Some(path) => path,
        None => {
            return deny_with_target(
                request,
                target,
                SteelMutationReasonCode::PathEscape,
                "mutation path escapes the repository-relative target boundary",
                Some(verb),
                None,
            );
        }
    };
    if !path_has_allowed_root(&normalized_path, &target.allowed_path_roots) {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::PathEscape,
            "mutation path is outside policy allowlisted roots",
            Some(verb),
            Some(normalized_path),
        );
    }
    if path_hits_denied_pattern(&normalized_path, &target.denied_path_patterns) {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::DeniedPathPattern,
            "mutation path matches a denied policy pattern",
            Some(verb),
            Some(normalized_path),
        );
    }
    if verb.writes_bytes && request.patch.is_none() {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::MissingPatch,
            "byte-writing mutation verb requires a patch descriptor",
            Some(verb),
            Some(normalized_path),
        );
    }
    if verb.requires_approval && !request.approval.approved {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::MissingApproval,
            "mutation verb requires explicit approval",
            Some(verb),
            Some(normalized_path),
        );
    }
    if verb.requires_approval && request.approval.tier != target.approval_tier {
        return deny_with_target(
            request,
            target,
            SteelMutationReasonCode::ApprovalTierMismatch,
            "approval tier does not match target policy",
            Some(verb),
            Some(normalized_path),
        );
    }
    let required_resource = format!("{}{}", target.resource_prefix, request.resource);
    let ucan = match authorize_ucan(policy, verb, &required_resource, &request.expected_audience, request.ucan.as_ref())
    {
        Ok(grant) => grant,
        Err((code, message, metadata)) => {
            return deny_with_target(request, target, code, message, Some(verb), Some(normalized_path))
                .with_safe_ucan_metadata(metadata);
        }
    };

    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Allowed,
        reason_code: SteelMutationReasonCode::Allowed,
        safe_message: "mutation request is authorized for Rust host preflight".to_string(),
        host_function: Some(verb.host_function.clone()),
        target_class: target.name.clone(),
        normalized_path: Some(normalized_path),
        required_ucan_ability: Some(verb.ucan_ability.clone()),
        required_ucan_resource: Some(required_resource),
        safe_ucan_metadata: Some(safe_ucan_metadata(ucan, SteelMutationDecisionOutcome::Allowed)),
        preflight_profile: Some(target.preflight_profile.clone()),
        verification_profile: Some(target.verification_profile.clone()),
        rollback_required: target.rollback_required,
    }
}

fn host_preflight_receipt(
    decision: SteelMutationDecision,
    context: &SteelMutationHostContext,
    checkpoint: SteelMutationCheckpointPlan,
    status: SteelMutationHostPreflightStatus,
    reason_code: SteelMutationHostPreflightReason,
    message: impl Into<String>,
) -> SteelMutationHostPreflightReceipt {
    let host_function = decision.host_function.clone();
    let normalized_path = decision.normalized_path.clone();
    let verification_profile = decision.verification_profile.clone();
    let safe_ucan_metadata = decision.safe_ucan_metadata.clone();
    SteelMutationHostPreflightReceipt {
        schema: STEEL_MUTATION_PREFLIGHT_SCHEMA.to_string(),
        status,
        reason_code,
        safe_message: message.into(),
        decision,
        host_function,
        normalized_path,
        policy_hash: context.policy_hash,
        target_hash: context.target_hash,
        checkpoint,
        verification_profile,
        safe_ucan_metadata,
        writes_performed: false,
    }
}

fn checkpoint_plan(
    decision: &SteelMutationDecision,
    context: &SteelMutationHostContext,
) -> SteelMutationCheckpointPlan {
    SteelMutationCheckpointPlan {
        required: decision.rollback_required,
        checkpoint_id: context.checkpoint_id.clone(),
        target_hash: context.target_hash,
        policy_hash: context.policy_hash,
    }
}

fn missing_required_capability(
    decision: &SteelMutationDecision,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> Option<&'static str> {
    let capabilities = context.session_capabilities.iter().map(String::as_str).collect::<BTreeSet<_>>();
    if !capabilities.contains(STEEL_MUTATION_SESSION_CAPABILITY) {
        return Some(STEEL_MUTATION_SESSION_CAPABILITY);
    }
    if decision.host_function.as_deref() != Some("steel.host.propose_mutation")
        && request.patch.is_some()
        && !capabilities.contains(WORKSPACE_MUTATION_SESSION_CAPABILITY)
    {
        return Some(WORKSPACE_MUTATION_SESSION_CAPABILITY);
    }
    None
}

fn host_function_is_disabled(
    decision: &SteelMutationDecision,
    request: &SteelMutationRequest,
    context: &SteelMutationHostContext,
) -> bool {
    let Some(host_function) = decision.host_function.as_deref() else {
        return false;
    };
    context.disabled_tools.iter().any(|disabled| {
        disabled == host_function
            || disabled == "steel.host.*"
            || disabled == "steel-self-mutation"
            || disabled == &request.verb
    })
}

fn target_class<'a>(policy: &'a SteelMutationPolicy, name: &str) -> Option<&'a SteelMutationTargetClass> {
    policy.target_classes.iter().find(|target| target.name == name)
}

fn verb_policy<'a>(policy: &'a SteelMutationPolicy, name: &str) -> Option<&'a SteelMutationVerbPolicy> {
    policy.mutation_verbs.iter().find(|verb| verb.name == name)
}

fn policy_is_safe(policy: &SteelMutationPolicy) -> bool {
    policy.schema == STEEL_MUTATION_POLICY_SCHEMA
        && policy.ucan.required
        && policy.ucan.deny_wildcard_resources
        && policy.receipt.schema == STEEL_MUTATION_RECEIPT_SCHEMA
        && policy.receipt.include_policy_hash
        && policy.receipt.include_safe_ucan_metadata
        && policy.runtime_profiles.iter().all(|profile| !profile.ambient_authority)
        && no_duplicate_names(policy.target_classes.iter().map(|target| target.name.as_str()))
        && no_duplicate_names(policy.mutation_verbs.iter().map(|verb| verb.name.as_str()))
}

fn no_duplicate_names<'a>(mut names: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeMap::new();
    names.all(|name| seen.insert(name, ()).is_none())
}

fn normalize_relative_path(path: &str) -> Option<String> {
    if path.is_empty() || path.starts_with('/') || path.contains('\0') {
        return None;
    }
    let mut normalized = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => return None,
            segment if segment.contains('\\') => return None,
            segment => normalized.push(segment),
        }
    }
    if normalized.is_empty() {
        None
    } else {
        Some(normalized.join("/"))
    }
}

fn path_has_allowed_root(path: &str, roots: &[String]) -> bool {
    roots.iter().any(|root| {
        let normalized_root = root.trim_start_matches("./");
        path == normalized_root.trim_end_matches('/') || path.starts_with(normalized_root)
    })
}

fn path_hits_denied_pattern(path: &str, denied_patterns: &[String]) -> bool {
    denied_patterns.iter().any(|pattern| match pattern.as_str() {
        "../" => path.split('/').any(|part| part == ".."),
        "/.git/" => path.contains("/.git/") || path.starts_with(".git/"),
        "**/.env*" => path.split('/').any(|part| part.starts_with(".env")),
        "**/*secret*" => path.to_ascii_lowercase().contains("secret"),
        pattern => path.contains(pattern.trim_matches('*')),
    })
}

fn authorize_ucan<'a>(
    policy: &SteelMutationPolicy,
    verb: &SteelMutationVerbPolicy,
    required_resource: &str,
    expected_audience: &str,
    grant: Option<&'a SteelMutationUcanGrant>,
) -> Result<&'a SteelMutationUcanGrant, (SteelMutationReasonCode, &'static str, Option<SteelMutationSafeUcanMetadata>)>
{
    let Some(grant) = grant else {
        return Err((SteelMutationReasonCode::MissingUcan, "mutation requires UCAN authority", None));
    };
    let denied_metadata = || safe_ucan_metadata(grant, SteelMutationDecisionOutcome::Denied);
    if grant.revoked {
        return Err((SteelMutationReasonCode::RevokedUcan, "UCAN grant is revoked", Some(denied_metadata())));
    }
    if grant.expiry_status != SteelMutationUcanExpiryStatus::Valid {
        return Err((SteelMutationReasonCode::ExpiredUcan, "UCAN grant is expired", Some(denied_metadata())));
    }
    if grant.ability != verb.ucan_ability {
        return Err((
            SteelMutationReasonCode::WrongUcanAbility,
            "UCAN ability does not authorize mutation verb",
            Some(denied_metadata()),
        ));
    }
    if grant.audience != expected_audience {
        return Err((
            SteelMutationReasonCode::WrongUcanAudience,
            "UCAN audience does not match mutation host context",
            Some(denied_metadata()),
        ));
    }
    if grant.delegation_depth > policy.ucan.max_delegation_depth {
        return Err((
            SteelMutationReasonCode::OverDelegatedUcan,
            "UCAN delegation depth exceeds mutation policy",
            Some(denied_metadata()),
        ));
    }
    if policy.ucan.deny_wildcard_resources && grant.resource == "*" {
        return Err((
            SteelMutationReasonCode::WildcardUcanResource,
            "wildcard UCAN resource is denied for live mutation",
            Some(denied_metadata()),
        ));
    }
    if grant.resource != required_resource {
        return Err((
            SteelMutationReasonCode::WrongUcanResource,
            "UCAN resource does not match mutation target",
            Some(denied_metadata()),
        ));
    }
    Ok(grant)
}

fn safe_ucan_metadata(
    grant: &SteelMutationUcanGrant,
    outcome: SteelMutationDecisionOutcome,
) -> SteelMutationSafeUcanMetadata {
    SteelMutationSafeUcanMetadata {
        ability: grant.ability.clone(),
        resource: grant.resource.clone(),
        audience: grant.audience.clone(),
        expiry_status: grant.expiry_status.clone(),
        delegation_depth: grant.delegation_depth,
        authorization_outcome: outcome,
    }
}

fn deny(
    request: &SteelMutationRequest,
    reason_code: SteelMutationReasonCode,
    message: impl Into<String>,
    host_function: Option<String>,
    required_ability: Option<String>,
    required_resource: Option<String>,
) -> SteelMutationDecision {
    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Denied,
        reason_code,
        safe_message: message.into(),
        host_function,
        target_class: request.target_class.clone(),
        normalized_path: None,
        required_ucan_ability: required_ability,
        required_ucan_resource: required_resource,
        safe_ucan_metadata: None,
        preflight_profile: None,
        verification_profile: None,
        rollback_required: false,
    }
}

fn deny_with_target(
    request: &SteelMutationRequest,
    target: &SteelMutationTargetClass,
    reason_code: SteelMutationReasonCode,
    message: impl Into<String>,
    verb: Option<&SteelMutationVerbPolicy>,
    normalized_path: Option<String>,
) -> SteelMutationDecision {
    let required_resource = verb.map(|_| format!("{}{}", target.resource_prefix, request.resource));
    let required_ability = verb.map(|policy| policy.ucan_ability.clone());
    SteelMutationDecision {
        schema: STEEL_MUTATION_DECISION_SCHEMA.to_string(),
        outcome: SteelMutationDecisionOutcome::Denied,
        reason_code,
        safe_message: message.into(),
        host_function: verb.map(|policy| policy.host_function.clone()),
        target_class: target.name.clone(),
        normalized_path,
        required_ucan_ability: required_ability,
        required_ucan_resource: required_resource,
        safe_ucan_metadata: None,
        preflight_profile: Some(target.preflight_profile.clone()),
        verification_profile: Some(target.verification_profile.clone()),
        rollback_required: target.rollback_required,
    }
}

impl SteelMutationDecision {
    #[must_use]
    fn with_safe_ucan_metadata(mut self, metadata: Option<SteelMutationSafeUcanMetadata>) -> Self {
        self.safe_ucan_metadata = metadata;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXPORTED_POLICY: &str = include_str!("../../../policy/steel-self-mutation/mutation-policy.json");
    const INVALID_POLICY: &str = include_str!("../../../policy/steel-self-mutation/invalid-policy.json");
    const TEST_REVIEWER: &str = "reviewer:test";
    const TEST_AUDIENCE: &str = "session:test";
    const TEST_PATCH_BYTES: u64 = 42;

    fn policy() -> SteelMutationPolicy {
        parse_steel_mutation_policy(EXPORTED_POLICY).expect("exported policy parses")
    }

    fn base_request() -> SteelMutationRequest {
        SteelMutationRequest {
            target_class: "prompt".to_string(),
            verb: "apply_mutation".to_string(),
            resource: "agent-system".to_string(),
            expected_audience: TEST_AUDIENCE.to_string(),
            relative_path: "crates/clankers-prompts/src/lib.rs".to_string(),
            intent: "tighten prompt fixture".to_string(),
            patch: Some(SteelMutationPatch {
                format: SteelMutationPatchFormat::UnifiedDiff,
                bytes: TEST_PATCH_BYTES,
                body_blake3: "b3:patch".to_string(),
            }),
            approval: SteelMutationApproval {
                approved: true,
                tier: "human-review".to_string(),
                reviewer: Some(TEST_REVIEWER.to_string()),
            },
            ucan: Some(SteelMutationUcanGrant {
                ability: "clankers/steel/mutation.apply".to_string(),
                resource: "prompt:agent-system".to_string(),
                audience: TEST_AUDIENCE.to_string(),
                expiry_status: SteelMutationUcanExpiryStatus::Valid,
                delegation_depth: 1,
                revoked: false,
            }),
        }
    }

    fn host_context() -> SteelMutationHostContext {
        SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([STEEL_MUTATION_SESSION_CAPABILITY, WORKSPACE_MUTATION_SESSION_CAPABILITY])
            .with_target_hash(ArtifactHash::digest(b"current target bytes"))
    }

    #[test]
    fn preflight_receipt_is_ready_without_writing_when_policy_ucan_and_session_capabilities_pass() {
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &host_context());

        assert_eq!(receipt.schema, STEEL_MUTATION_PREFLIGHT_SCHEMA);
        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Ready);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::Ready);
        assert_eq!(receipt.host_function.as_deref(), Some("steel.host.apply_mutation"));
        assert_eq!(receipt.normalized_path.as_deref(), Some("crates/clankers-prompts/src/lib.rs"));
        assert_eq!(receipt.verification_profile.as_deref(), Some("prompt-schema-and-smoke"));
        assert!(receipt.checkpoint.required);
        assert_eq!(receipt.checkpoint.target_hash, host_context().target_hash);
        assert!(!receipt.writes_performed);
        assert!(receipt.receipt_hash().prefixed().starts_with("b3:"));
    }

    #[test]
    fn preflight_blocks_when_session_lacks_mutation_capability() {
        let context = SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([WORKSPACE_MUTATION_SESSION_CAPABILITY])
            .with_target_hash(ArtifactHash::digest(b"current target bytes"));
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &context);

        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::MissingSessionCapability);
        assert_eq!(receipt.decision.outcome, SteelMutationDecisionOutcome::Allowed);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn preflight_blocks_disabled_host_function_and_dirty_uncheckpointed_repo() {
        let disabled_context = host_context().with_disabled_tools(["steel.host.apply_mutation"]);
        let disabled = preflight_steel_mutation_host_function(&policy(), &base_request(), &disabled_context);
        assert_eq!(disabled.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(disabled.reason_code, SteelMutationHostPreflightReason::DisabledHostFunction);

        let dirty_context = host_context().with_dirty_repository(None);
        let dirty = preflight_steel_mutation_host_function(&policy(), &base_request(), &dirty_context);
        assert_eq!(dirty.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(dirty.reason_code, SteelMutationHostPreflightReason::DirtyRepositoryNeedsCheckpoint);
        assert_eq!(dirty.checkpoint.checkpoint_id, None);
    }

    #[test]
    fn preflight_blocks_rollback_required_request_without_target_hash() {
        let context = SteelMutationHostContext::new(ArtifactHash::digest(EXPORTED_POLICY.as_bytes()))
            .with_session_capabilities([STEEL_MUTATION_SESSION_CAPABILITY, WORKSPACE_MUTATION_SESSION_CAPABILITY]);
        let receipt = preflight_steel_mutation_host_function(&policy(), &base_request(), &context);

        assert_eq!(receipt.status, SteelMutationHostPreflightStatus::Blocked);
        assert_eq!(receipt.reason_code, SteelMutationHostPreflightReason::MissingTargetHash);
        assert!(receipt.checkpoint.required);
        assert_eq!(receipt.target_hash, None);
        assert!(!receipt.writes_performed);
    }

    #[test]
    fn prompt_apply_request_is_authorized_for_rust_host_preflight() {
        let decision = authorize_steel_mutation(&policy(), &base_request());

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Allowed);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::Allowed);
        assert_eq!(decision.host_function.as_deref(), Some("steel.host.apply_mutation"));
        assert_eq!(decision.normalized_path.as_deref(), Some("crates/clankers-prompts/src/lib.rs"));
        assert_eq!(decision.required_ucan_ability.as_deref(), Some("clankers/steel/mutation.apply"));
        assert_eq!(decision.required_ucan_resource.as_deref(), Some("prompt:agent-system"));
        assert_eq!(decision.verification_profile.as_deref(), Some("prompt-schema-and-smoke"));
        assert!(decision.rollback_required);
        assert_eq!(
            decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
            Some(&SteelMutationDecisionOutcome::Allowed)
        );
    }

    #[test]
    fn raw_path_escape_is_denied_before_ucan_success() {
        let mut request = base_request();
        request.relative_path = "../secrets.env".to_string();
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::PathEscape);
        assert_eq!(decision.safe_ucan_metadata, None);
    }

    #[test]
    fn wildcard_ucan_resource_is_denied() {
        let mut request = base_request();
        request.ucan.as_mut().expect("ucan").resource = "*".to_string();
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::WildcardUcanResource);
        assert_eq!(
            decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
            Some(&SteelMutationDecisionOutcome::Denied)
        );
    }

    #[test]
    fn expired_revoked_wrong_audience_and_over_delegated_ucans_are_denied() {
        for (mut grant, expected) in [
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Expired,
                    delegation_depth: 1,
                    revoked: false,
                },
                SteelMutationReasonCode::ExpiredUcan,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 1,
                    revoked: true,
                },
                SteelMutationReasonCode::RevokedUcan,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: "session:other".to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 1,
                    revoked: false,
                },
                SteelMutationReasonCode::WrongUcanAudience,
            ),
            (
                SteelMutationUcanGrant {
                    ability: "clankers/steel/mutation.apply".to_string(),
                    resource: "prompt:agent-system".to_string(),
                    audience: TEST_AUDIENCE.to_string(),
                    expiry_status: SteelMutationUcanExpiryStatus::Valid,
                    delegation_depth: 2,
                    revoked: false,
                },
                SteelMutationReasonCode::OverDelegatedUcan,
            ),
        ] {
            let mut request = base_request();
            request.ucan = Some(grant.clone());
            let decision = authorize_steel_mutation(&policy(), &request);

            assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
            assert_eq!(decision.reason_code, expected);
            assert_eq!(
                decision.safe_ucan_metadata.as_ref().map(|metadata| &metadata.authorization_outcome),
                Some(&SteelMutationDecisionOutcome::Denied)
            );
            grant.revoked = false;
        }
    }

    #[test]
    fn wrong_ability_and_wrong_resource_ucans_are_denied() {
        for (field, expected) in [
            ("ability", SteelMutationReasonCode::WrongUcanAbility),
            ("resource", SteelMutationReasonCode::WrongUcanResource),
        ] {
            let mut request = base_request();
            let ucan = request.ucan.as_mut().expect("ucan");
            if field == "ability" {
                ucan.ability = "clankers/steel/mutation.propose".to_string();
            } else {
                ucan.resource = "prompt:other".to_string();
            }
            let decision = authorize_steel_mutation(&policy(), &request);

            assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
            assert_eq!(decision.reason_code, expected);
        }
    }

    #[test]
    fn missing_approval_is_denied_for_byte_writing_apply() {
        let mut request = base_request();
        request.approval.approved = false;
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::MissingApproval);
    }

    #[test]
    fn missing_patch_is_denied_for_byte_writing_apply() {
        let mut request = base_request();
        request.patch = None;
        let decision = authorize_steel_mutation(&policy(), &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::MissingPatch);
    }

    #[test]
    fn unsafe_exported_policy_fixture_fails_closed() {
        let unsafe_policy = parse_steel_mutation_policy(INVALID_POLICY).expect("invalid fixture still parses as DTO");
        let mut request = base_request();
        request.target_class = "repo_code".to_string();
        request.verb = "raw_write".to_string();
        request.relative_path = "src/main.rs".to_string();
        request.resource = "src/main.rs".to_string();
        request.ucan.as_mut().expect("ucan").ability = "*".to_string();
        request.ucan.as_mut().expect("ucan").resource = "*".to_string();

        let decision = authorize_steel_mutation(&unsafe_policy, &request);

        assert_eq!(decision.outcome, SteelMutationDecisionOutcome::Denied);
        assert_eq!(decision.reason_code, SteelMutationReasonCode::InvalidPolicy);
    }
}
