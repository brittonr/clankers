//! SOUL/personality first-pass policy helpers.
//!
//! This module validates local SOUL.md and personality-preset intent without
//! reading raw prompt contents, fetching remote personas, or mutating the active
//! system prompt.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SoulSource {
    Discovery,
    LocalFile { label: String },
    Remote { kind: String },
    Command,
}

impl SoulSource {
    pub fn kind(&self) -> &'static str {
        match self {
            SoulSource::Discovery => "discovery",
            SoulSource::LocalFile { .. } => "local_file",
            SoulSource::Remote { .. } => "remote",
            SoulSource::Command => "command",
        }
    }

    pub fn label(&self) -> String {
        match self {
            SoulSource::Discovery => "SOUL.md".to_string(),
            SoulSource::LocalFile { label } => label.clone(),
            SoulSource::Remote { kind } => kind.clone(),
            SoulSource::Command => "command".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PersonalityPreset {
    pub label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoulValidation {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
    pub soul_kind: String,
    pub soul_label: String,
    pub personality: Option<String>,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

pub fn parse_soul_source(input: Option<&str>) -> SoulSource {
    let Some(input) = input else {
        return SoulSource::Discovery;
    };
    let normalized = input.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("discover") {
        return SoulSource::Discovery;
    }
    let lower = normalized.to_ascii_lowercase();
    if lower.starts_with("http://") {
        return SoulSource::Remote {
            kind: "http".to_string(),
        };
    }
    if lower.starts_with("https://") {
        return SoulSource::Remote {
            kind: "https".to_string(),
        };
    }
    if lower.starts_with("s3://") || lower.starts_with("cloud:") {
        return SoulSource::Remote {
            kind: "cloud".to_string(),
        };
    }
    if lower.starts_with("sh:") || lower.starts_with("cmd:") || normalized.contains("$(") || normalized.contains('`') {
        return SoulSource::Command;
    }

    let path = normalized.strip_prefix("file:").unwrap_or(normalized);
    SoulSource::LocalFile {
        label: safe_file_label(path),
    }
}

pub fn parse_personality(input: Option<&str>) -> Result<Option<PersonalityPreset>, String> {
    let Some(input) = input else {
        return Ok(None);
    };
    let normalized = input.trim();
    if normalized.is_empty() || normalized.eq_ignore_ascii_case("none") {
        return Ok(None);
    }
    if normalized.len() > 64 {
        return Err("personality preset name is too long".to_string());
    }
    let valid = normalized.chars().all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'));
    if !valid {
        return Err("personality preset name must use only ASCII letters, numbers, '.', '-', or '_'".to_string());
    }
    Ok(Some(PersonalityPreset {
        label: normalized.to_string(),
    }))
}

pub fn validate(source: &SoulSource, personality: Option<&PersonalityPreset>) -> SoulValidation {
    let validation = SoulValidation {
        source: "soul_personality",
        action: "validate",
        status: "success",
        backend: "local-policy",
        soul_kind: source.kind().to_string(),
        soul_label: source.label(),
        personality: personality.map(|preset| preset.label.clone()),
        supported: true,
        error_kind: None,
        error_message: None,
    };

    match source {
        SoulSource::Discovery | SoulSource::LocalFile { .. } => validation,
        SoulSource::Remote { kind } => unsupported(
            validation,
            "unsupported_source",
            &format!("remote SOUL/personality source '{kind}' is not supported in the first pass"),
        ),
        SoulSource::Command => unsupported(
            validation,
            "unsupported_source",
            "command-based SOUL/personality sources are not supported in the first pass",
        ),
    }
}

pub fn status_summary() -> SoulValidation {
    validate(&SoulSource::Discovery, None)
}

fn safe_file_label(path: &str) -> String {
    let name = Path::new(path)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("SOUL.md");
    sanitize_label(name)
}

fn unsupported(mut validation: SoulValidation, kind: &'static str, message: &str) -> SoulValidation {
    validation.status = "unsupported";
    validation.supported = false;
    validation.error_kind = Some(kind);
    validation.error_message = Some(sanitize_label(message));
    validation
}

fn sanitize_label(message: &str) -> String {
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
    fn parses_discovery_and_local_file_without_preserving_full_path() {
        assert_eq!(parse_soul_source(None), SoulSource::Discovery);
        assert_eq!(parse_soul_source(Some("file:/tmp/private/SOUL.md")).label(), "SOUL.md");
    }

    #[test]
    fn parses_remote_and_command_sources_as_safe_kinds() {
        assert_eq!(parse_soul_source(Some("https://token@example.test/SOUL.md")).label(), "https");
        assert_eq!(parse_soul_source(Some("s3://bucket/SOUL.md")).label(), "cloud");
        assert_eq!(parse_soul_source(Some("cmd:cat /tmp/SOUL.md")).label(), "command");
    }

    #[test]
    fn validates_personality_names() {
        assert_eq!(parse_personality(None).unwrap(), None);
        assert_eq!(parse_personality(Some("mentor.v1")).unwrap().unwrap().label, "mentor.v1");
        assert!(parse_personality(Some("../secret")).unwrap_err().contains("ASCII"));
        assert!(parse_personality(Some("bad name")).unwrap_err().contains("ASCII"));
    }

    #[test]
    fn validates_local_source_and_rejects_remote_first_pass() {
        let personality = parse_personality(Some("concise")).unwrap();
        let local = validate(&parse_soul_source(Some("./SOUL.md")), personality.as_ref());
        assert!(local.supported);
        assert_eq!(local.soul_label, "SOUL.md");
        assert_eq!(local.personality.as_deref(), Some("concise"));

        let remote = validate(&parse_soul_source(Some("https://token@example.test/SOUL.md\nsecret")), None);
        assert!(!remote.supported);
        assert_eq!(remote.soul_label, "https");
        assert_eq!(remote.error_kind, Some("unsupported_source"));
        let message = remote.error_message.expect("message");
        assert!(!message.contains('\n'));
        assert!(!message.contains("token@example.test"));
    }
}
