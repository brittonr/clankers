//! Tool gateway and platform delivery policy helpers.
//!
//! The first pass is intentionally a validation/metadata boundary. It keeps
//! toolset names and delivery targets explicit before later platform backends
//! grow beyond local/session delivery.

use serde::Serialize;

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

fn unsupported(mut validation: GatewayValidation, kind: &'static str, message: &str) -> GatewayValidation {
    validation.status = "unsupported";
    validation.supported = false;
    validation.error_kind = Some(kind);
    validation.error_message = Some(sanitize_error_message(message));
    validation
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
    use super::*;

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
}
