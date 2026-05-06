//! Tool gateway and platform delivery policy helpers.
//!
//! The gateway is intentionally a validation/metadata boundary. It keeps
//! toolset names, disabled-tool filtering, and delivery receipts explicit so
//! standalone, daemon, and platform paths do not grow ad hoc policy forks.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlatformDeliveryReceipt {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub artifact_type: &'static str,
    pub backend: &'static str,
    pub target_kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub safe_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform_handle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub redaction: &'static str,
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
    let artifact_type = kind.as_str();
    match target {
        DeliveryTarget::Local | DeliveryTarget::Session => PlatformDeliveryReceipt {
            source: "tool_gateway",
            action: "deliver",
            status: "success",
            artifact_type,
            backend: "local",
            target_kind: target.as_label().to_string(),
            safe_path: path.map(safe_path_label),
            platform_handle: None,
            error_kind: None,
            error_message: None,
            redaction: "safe_metadata_only",
        },
        DeliveryTarget::Matrix => {
            delivery_unsupported(artifact_type, "matrix", "matrix delivery requires an active platform bridge adapter")
        }
        DeliveryTarget::Unsupported { kind } => delivery_unsupported(
            artifact_type,
            kind,
            &format!("delivery target '{kind}' is not supported by the local delivery adapter"),
        ),
    }
}

fn unsupported(mut validation: GatewayValidation, kind: &'static str, message: &str) -> GatewayValidation {
    validation.status = "unsupported";
    validation.supported = false;
    validation.error_kind = Some(kind);
    validation.error_message = Some(sanitize_error_message(message));
    validation
}

fn delivery_unsupported(artifact_type: &'static str, target_kind: &str, message: &str) -> PlatformDeliveryReceipt {
    PlatformDeliveryReceipt {
        source: "tool_gateway",
        action: "deliver",
        status: "unsupported",
        artifact_type,
        backend: "local",
        target_kind: target_kind.to_string(),
        safe_path: None,
        platform_handle: None,
        error_kind: Some("unsupported_target"),
        error_message: Some(sanitize_error_message(message)),
        redaction: "safe_metadata_only",
    }
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
