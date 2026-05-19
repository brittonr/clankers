//! Tool gateway and platform delivery policy helpers.
//!
//! The gateway is intentionally a validation/metadata boundary. It keeps
//! toolset names, disabled-tool filtering, and delivery receipts explicit so
//! standalone, daemon, and platform paths do not grow ad hoc policy forks.

use std::collections::HashSet;
use std::fs;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use serde::Deserialize;
use serde::Serialize;

use crate::modes::common::ToolTier;
use crate::tools::Tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayToolset {
    Core,
    Orchestration,
    Specialty,
    Matrix,
}

impl GatewayToolset {
    pub fn as_str(self) -> &'static str {
        match self {
            GatewayToolset::Core => "core",
            GatewayToolset::Orchestration => "orchestration",
            GatewayToolset::Specialty => "specialty",
            GatewayToolset::Matrix => "matrix",
        }
    }
}

impl From<ToolTier> for GatewayToolset {
    fn from(value: ToolTier) -> Self {
        match value {
            ToolTier::Core => GatewayToolset::Core,
            ToolTier::Orchestration => GatewayToolset::Orchestration,
            ToolTier::Specialty => GatewayToolset::Specialty,
            ToolTier::Matrix => GatewayToolset::Matrix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryTarget {
    Local,
    Session,
    Matrix,
    Unsupported { kind: String },
}

impl DeliveryTarget {
    pub fn as_label(&self) -> &str {
        match self {
            DeliveryTarget::Local => "local",
            DeliveryTarget::Session => "session",
            DeliveryTarget::Matrix => "matrix",
            DeliveryTarget::Unsupported { kind } => kind.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayValidation {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
    pub toolsets: Vec<String>,
    pub delivery_target: String,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GatewayMode {
    Standalone,
    Daemon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GatewayToolPolicyReceipt {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub mode: GatewayMode,
    pub active_toolsets: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub allowed_tools: Vec<String>,
    pub allowed_count: usize,
    pub redaction: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactKind {
    File,
    Media,
    ScheduledOutput,
}

impl ArtifactKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ArtifactKind::File => "file",
            ArtifactKind::Media => "media",
            ArtifactKind::ScheduledOutput => "scheduled_output",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformDeliveryReceipt {
    pub source: String,
    pub action: String,
    pub status: String,
    pub attempt_id: String,
    pub artifact_type: String,
    pub backend: String,
    pub target_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub retryable: bool,
    pub redaction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryContext {
    pub matrix_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub matrix_binding: Option<String>,
}

impl DeliveryContext {
    pub fn local() -> Self {
        Self {
            matrix_active: false,
            matrix_binding: None,
        }
    }

    pub fn matrix(binding: impl Into<String>) -> Self {
        Self {
            matrix_active: true,
            matrix_binding: Some(safe_handle_label(&binding.into())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryAttempt {
    pub source: String,
    pub action: String,
    pub attempt_id: String,
    pub status: String,
    pub artifact_type: String,
    pub target_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_path: Option<String>,
    pub retryable: bool,
    pub receipt: PlatformDeliveryReceipt,
    pub redaction: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeliveryOutbox {
    pub source: String,
    pub attempts: Vec<DeliveryAttempt>,
    pub redaction: String,
}

impl Default for DeliveryOutbox {
    fn default() -> Self {
        Self {
            source: "tool_gateway".to_string(),
            attempts: Vec::new(),
            redaction: "safe_metadata_only".to_string(),
        }
    }
}

pub trait DeliveryAdapter {
    fn backend(&self) -> &'static str;
    fn deliver(&self, request: &DeliveryRequest) -> PlatformDeliveryReceipt;
}

#[derive(Debug, Clone)]
pub struct DeliveryRequest {
    pub kind: ArtifactKind,
    pub path: Option<PathBuf>,
    pub target: DeliveryTarget,
    pub context: DeliveryContext,
}

pub fn parse_toolsets(input: &str) -> Result<Vec<GatewayToolset>, String> {
    let mut parsed = Vec::new();
    for raw in input.split(',') {
        let name = raw.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        let toolset = match name.as_str() {
            "core" => GatewayToolset::Core,
            "orchestration" | "orch" => GatewayToolset::Orchestration,
            "specialty" | "spec" => GatewayToolset::Specialty,
            "matrix" => GatewayToolset::Matrix,
            other => return Err(format!("unknown toolset '{other}'")),
        };
        if !parsed.contains(&toolset) {
            parsed.push(toolset);
        }
    }
    if parsed.is_empty() {
        return Err("at least one toolset is required".to_string());
    }
    Ok(parsed)
}

pub fn parse_delivery_target(input: Option<&str>) -> DeliveryTarget {
    let Some(raw) = input else {
        return DeliveryTarget::Local;
    };
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "" | "local" => DeliveryTarget::Local,
        "session" => DeliveryTarget::Session,
        "matrix" => DeliveryTarget::Matrix,
        other => {
            let kind = other.split_once(':').map(|(prefix, _)| prefix).unwrap_or(other);
            DeliveryTarget::Unsupported { kind: kind.to_string() }
        }
    }
}

pub fn validate(toolsets: &[GatewayToolset], target: &DeliveryTarget, matrix_active: bool) -> GatewayValidation {
    let toolset_labels = toolsets.iter().map(|toolset| toolset.as_str().to_string()).collect();
    let mut validation = GatewayValidation {
        source: "tool_gateway",
        action: "validate",
        status: "success",
        backend: "local",
        toolsets: toolset_labels,
        delivery_target: target.as_label().to_string(),
        supported: true,
        error_kind: None,
        error_message: None,
    };

    match target {
        DeliveryTarget::Local | DeliveryTarget::Session => validation,
        DeliveryTarget::Matrix if matrix_active => {
            validation.backend = "matrix-existing-bridge";
            validation
        }
        DeliveryTarget::Matrix => {
            unsupported(validation, "unsupported_target", "matrix delivery requires an active Matrix bridge session")
        }
        DeliveryTarget::Unsupported { kind } => unsupported(
            validation,
            "unsupported_target",
            &format!("delivery target '{kind}' is not supported in the first-pass local gateway"),
        ),
    }
}

pub fn status_summary() -> GatewayValidation {
    validate(
        &[
            GatewayToolset::Core,
            GatewayToolset::Orchestration,
            GatewayToolset::Specialty,
            GatewayToolset::Matrix,
        ],
        &DeliveryTarget::Local,
        false,
    )
}

pub fn standalone_toolsets() -> [ToolTier; 3] {
    [ToolTier::Core, ToolTier::Specialty, ToolTier::Orchestration]
}

pub fn daemon_toolsets() -> [ToolTier; 4] {
    [
        ToolTier::Core,
        ToolTier::Orchestration,
        ToolTier::Specialty,
        ToolTier::Matrix,
    ]
}

pub fn allowed_tools_for_policy(
    tiered_tools: &[(ToolTier, Arc<dyn Tool>)],
    active_tiers: &[ToolTier],
    disabled_tools: &HashSet<String>,
) -> Vec<Arc<dyn Tool>> {
    let active: HashSet<ToolTier> = active_tiers.iter().copied().collect();
    tiered_tools
        .iter()
        .filter(|(tier, tool)| active.contains(tier) && !disabled_tools.contains(&tool.definition().name))
        .map(|(_, tool)| tool.clone())
        .collect()
}

pub fn tool_policy_receipt(
    mode: GatewayMode,
    active_tiers: &[ToolTier],
    disabled_tools: &HashSet<String>,
    allowed_tools: &[Arc<dyn Tool>],
) -> GatewayToolPolicyReceipt {
    let mut active_toolsets: Vec<String> = active_tiers
        .iter()
        .copied()
        .map(GatewayToolset::from)
        .map(|toolset| toolset.as_str().to_string())
        .collect();
    active_toolsets.sort();
    active_toolsets.dedup();

    let mut disabled_tools: Vec<String> = disabled_tools.iter().cloned().collect();
    disabled_tools.sort();

    let mut allowed_tool_names: Vec<String> = allowed_tools.iter().map(|tool| tool.definition().name.clone()).collect();
    allowed_tool_names.sort();

    GatewayToolPolicyReceipt {
        source: "tool_gateway",
        action: "tool_policy",
        status: "success",
        mode,
        active_toolsets,
        disabled_tools,
        allowed_count: allowed_tool_names.len(),
        allowed_tools: allowed_tool_names,
        redaction: "safe_metadata_only",
    }
}

pub fn local_delivery_receipt(
    kind: ArtifactKind,
    path: Option<&Path>,
    target: &DeliveryTarget,
) -> PlatformDeliveryReceipt {
    deliver_artifact(kind, path, target, &DeliveryContext::local()).receipt
}

pub fn deliver_artifact(
    kind: ArtifactKind,
    path: Option<&Path>,
    target: &DeliveryTarget,
    context: &DeliveryContext,
) -> DeliveryAttempt {
    let request = DeliveryRequest {
        kind,
        path: path.map(Path::to_path_buf),
        target: target.clone(),
        context: context.clone(),
    };
    let receipt = match target {
        DeliveryTarget::Local | DeliveryTarget::Session => LocalDeliveryAdapter.deliver(&request),
        DeliveryTarget::Matrix if context.matrix_active => MatrixDeliveryAdapter.deliver(&request),
        DeliveryTarget::Matrix => delivery_unsupported(
            kind.as_str(),
            "matrix",
            "matrix delivery requires an active Matrix bridge session",
            false,
        ),
        DeliveryTarget::Unsupported { kind: target_kind } => delivery_unsupported(
            kind.as_str(),
            target_kind,
            &format!("delivery target '{target_kind}' is not supported by configured gateway adapters"),
            false,
        ),
    };
    DeliveryAttempt {
        source: "tool_gateway".to_string(),
        action: "deliver_attempt".to_string(),
        attempt_id: receipt.attempt_id.clone(),
        status: receipt.status.clone(),
        artifact_type: receipt.artifact_type.clone(),
        target_kind: receipt.target_kind.clone(),
        safe_path: receipt.safe_path.clone(),
        retryable: receipt.retryable,
        receipt,
        redaction: "safe_metadata_only".to_string(),
    }
}

pub fn read_outbox(path: &Path) -> Result<DeliveryOutbox, String> {
    if !path.exists() {
        return Ok(DeliveryOutbox::default());
    }
    let data = fs::read_to_string(path).map_err(|err| format!("read outbox failed: {err}"))?;
    if data.trim().is_empty() {
        return Ok(DeliveryOutbox::default());
    }
    serde_json::from_str(&data).map_err(|err| format!("parse outbox failed: {err}"))
}

pub fn write_outbox(path: &Path, outbox: &DeliveryOutbox) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| format!("create outbox directory failed: {err}"))?;
    }
    let data = serde_json::to_string_pretty(outbox).map_err(|err| format!("serialize outbox failed: {err}"))?;
    fs::write(path, format!("{data}\n")).map_err(|err| format!("write outbox failed: {err}"))
}

pub fn record_attempt(path: &Path, attempt: DeliveryAttempt) -> Result<DeliveryAttempt, String> {
    let mut outbox = read_outbox(path)?;
    outbox.attempts.retain(|existing| existing.attempt_id != attempt.attempt_id);
    outbox.attempts.push(attempt.clone());
    write_outbox(path, &outbox)?;
    Ok(attempt)
}

pub fn find_attempt(path: &Path, attempt_id: &str) -> Result<DeliveryAttempt, String> {
    let outbox = read_outbox(path)?;
    outbox
        .attempts
        .into_iter()
        .find(|attempt| attempt.attempt_id == attempt_id)
        .ok_or_else(|| "delivery attempt not found".to_string())
}

pub fn retry_attempt(path: &Path, attempt_id: &str, context: &DeliveryContext) -> Result<DeliveryAttempt, String> {
    let prior = find_attempt(path, attempt_id)?;
    if !prior.retryable {
        return Err("delivery attempt is not retryable".to_string());
    }
    let kind = parse_artifact_kind_label(&prior.artifact_type)?;
    let target = parse_delivery_target(Some(&prior.target_kind));
    let retry = deliver_artifact(kind, prior.safe_path.as_deref().map(Path::new), &target, context);
    record_attempt(path, retry)
}

fn unsupported(mut validation: GatewayValidation, kind: &'static str, message: &str) -> GatewayValidation {
    validation.status = "unsupported";
    validation.supported = false;
    validation.error_kind = Some(kind);
    validation.error_message = Some(sanitize_error_message(message));
    validation
}

fn delivery_unsupported(
    artifact_type: &'static str,
    target_kind: &str,
    message: &str,
    retryable: bool,
) -> PlatformDeliveryReceipt {
    PlatformDeliveryReceipt {
        source: "tool_gateway".to_string(),
        action: "deliver".to_string(),
        status: "unsupported".to_string(),
        attempt_id: attempt_id(artifact_type, target_kind, None),
        artifact_type: artifact_type.to_string(),
        backend: "policy".to_string(),
        target_kind: target_kind.to_string(),
        safe_path: None,
        platform_handle: None,
        error_kind: Some("unsupported_target".to_string()),
        error_message: Some(sanitize_error_message(message)),
        retryable,
        redaction: "safe_metadata_only".to_string(),
    }
}

struct LocalDeliveryAdapter;

impl DeliveryAdapter for LocalDeliveryAdapter {
    fn backend(&self) -> &'static str {
        "local"
    }

    fn deliver(&self, request: &DeliveryRequest) -> PlatformDeliveryReceipt {
        let safe_path = request.path.as_deref().map(safe_path_label);
        PlatformDeliveryReceipt {
            source: "tool_gateway".to_string(),
            action: "deliver".to_string(),
            status: "success".to_string(),
            attempt_id: attempt_id(request.kind.as_str(), request.target.as_label(), safe_path.as_deref()),
            artifact_type: request.kind.as_str().to_string(),
            backend: self.backend().to_string(),
            target_kind: request.target.as_label().to_string(),
            safe_path,
            platform_handle: None,
            error_kind: None,
            error_message: None,
            retryable: false,
            redaction: "safe_metadata_only".to_string(),
        }
    }
}

struct MatrixDeliveryAdapter;

impl DeliveryAdapter for MatrixDeliveryAdapter {
    fn backend(&self) -> &'static str {
        "matrix"
    }

    fn deliver(&self, request: &DeliveryRequest) -> PlatformDeliveryReceipt {
        let safe_path = request.path.as_deref().map(safe_path_label);
        let handle_seed = request.context.matrix_binding.as_deref().unwrap_or("active_matrix_session");
        PlatformDeliveryReceipt {
            source: "tool_gateway".to_string(),
            action: "deliver".to_string(),
            status: "success".to_string(),
            attempt_id: attempt_id(request.kind.as_str(), "matrix", safe_path.as_deref()),
            artifact_type: request.kind.as_str().to_string(),
            backend: self.backend().to_string(),
            target_kind: "matrix".to_string(),
            safe_path,
            platform_handle: Some(format!("matrix:{}", short_hash(handle_seed))),
            error_kind: None,
            error_message: None,
            retryable: false,
            redaction: "safe_metadata_only".to_string(),
        }
    }
}

fn parse_artifact_kind_label(label: &str) -> Result<ArtifactKind, String> {
    match label {
        "file" => Ok(ArtifactKind::File),
        "media" => Ok(ArtifactKind::Media),
        "scheduled_output" | "scheduled-output" | "scheduled" => Ok(ArtifactKind::ScheduledOutput),
        other => Err(format!("unknown artifact type '{other}'")),
    }
}

fn attempt_id(artifact_type: &str, target_kind: &str, safe_path: Option<&str>) -> String {
    short_hash(&format!("{artifact_type}:{target_kind}:{}", safe_path.unwrap_or("artifact")))
}

fn short_hash(value: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::Hasher;
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn safe_handle_label(value: &str) -> String {
    value
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'))
        .take(64)
        .collect::<String>()
}

fn safe_path_label(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("artifact")
        .chars()
        .filter(|ch| !matches!(ch, '\n' | '\r'))
        .take(120)
        .collect()
}

fn sanitize_error_message(message: &str) -> String {
    let flattened = message.replace(['\n', '\r'], " ");
    let mut chars = flattened.chars();
    let truncated: String = chars.by_ref().take(240).collect();
    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;
    use std::sync::Arc;

    use super::*;
    use crate::tools::Tool;
    use crate::tools::ToolContext;
    use crate::tools::ToolDefinition;
    use crate::tools::ToolResult;

    struct FakeTool {
        definition: ToolDefinition,
    }

    #[async_trait::async_trait]
    impl Tool for FakeTool {
        fn definition(&self) -> &ToolDefinition {
            &self.definition
        }

        async fn execute(&self, _ctx: &ToolContext, _params: serde_json::Value) -> ToolResult {
            ToolResult::text("fake")
        }
    }

    fn fake_tool(name: &str) -> Arc<dyn Tool> {
        Arc::new(FakeTool {
            definition: ToolDefinition {
                name: name.to_string(),
                description: "fake tool".to_string(),
                input_schema: serde_json::json!({"type":"object"}),
            },
        })
    }

    #[test]
    fn parses_known_toolsets_and_deduplicates_aliases() {
        let parsed = parse_toolsets("core, spec, orchestration, specialty").expect("parse toolsets");
        assert_eq!(parsed, vec![
            GatewayToolset::Core,
            GatewayToolset::Specialty,
            GatewayToolset::Orchestration
        ]);
    }

    #[test]
    fn rejects_unknown_or_empty_toolsets() {
        assert!(parse_toolsets("core,unknown").unwrap_err().contains("unknown toolset"));
        assert!(parse_toolsets(" , ").unwrap_err().contains("at least one"));
    }

    #[test]
    fn validates_local_and_matrix_boundaries() {
        let toolsets = parse_toolsets("core").expect("toolsets");
        assert!(validate(&toolsets, &parse_delivery_target(Some("local")), false).supported);

        let matrix = validate(&toolsets, &parse_delivery_target(Some("matrix")), false);
        assert!(!matrix.supported);
        assert_eq!(matrix.status, "unsupported");
        assert_eq!(matrix.error_kind, Some("unsupported_target"));

        let active_matrix = validate(&toolsets, &parse_delivery_target(Some("matrix")), true);
        assert!(active_matrix.supported);
        assert_eq!(active_matrix.backend, "matrix-existing-bridge");
    }

    #[test]
    fn unsupported_targets_are_replay_safe() {
        let toolsets = parse_toolsets("core").expect("toolsets");
        let result =
            validate(&toolsets, &parse_delivery_target(Some("https://token@example.test/hook\nsecret")), false);
        assert!(!result.supported);
        assert_eq!(result.delivery_target, "https");
        let message = result.error_message.expect("error message");
        assert!(!message.contains('\n'));
        assert!(message.chars().count() <= 241);
    }

    #[test]
    fn shared_tool_policy_filters_disabled_tools_with_safe_receipt() {
        let tools = vec![
            (ToolTier::Core, fake_tool("read")),
            (ToolTier::Specialty, fake_tool("web")),
            (ToolTier::Matrix, fake_tool("matrix_send")),
        ];
        let disabled = HashSet::from(["web".to_string()]);
        let allowed = allowed_tools_for_policy(&tools, &standalone_toolsets(), &disabled);
        let receipt = tool_policy_receipt(GatewayMode::Standalone, &standalone_toolsets(), &disabled, &allowed);

        assert_eq!(allowed.iter().map(|tool| tool.definition().name.as_str()).collect::<Vec<_>>(), vec!["read"]);
        assert_eq!(receipt.allowed_tools, vec!["read".to_string()]);
        assert_eq!(receipt.disabled_tools, vec!["web".to_string()]);
        assert_eq!(receipt.redaction, "safe_metadata_only");
    }

    #[test]
    fn delivery_receipts_record_safe_path_or_unsupported_kind() {
        let local = local_delivery_receipt(
            ArtifactKind::Media,
            Some(Path::new("/tmp/token/voice-output.mp3")),
            &DeliveryTarget::Session,
        );
        assert_eq!(local.status, "success");
        assert_eq!(local.safe_path.as_deref(), Some("voice-output.mp3"));

        let remote = local_delivery_receipt(
            ArtifactKind::File,
            Some(Path::new("/tmp/secret.txt")),
            &parse_delivery_target(Some("https://token@example.test/hook")),
        );
        assert_eq!(remote.status, "unsupported");
        assert_eq!(remote.target_kind, "https");
        assert!(remote.safe_path.is_none());
        assert!(!serde_json::to_string(&remote).expect("serialize").contains("token@example"));
    }
}
