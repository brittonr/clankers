//! Voice and speech-to-text first-pass policy helpers.
//!
//! This module validates voice/STT intent without capturing microphone input,
//! reading raw audio, or contacting transcription providers.

use std::path::Path;

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceInputSource {
    File { extension: Option<String> },
    Microphone,
    Matrix,
    Remote { kind: String },
}

impl VoiceInputSource {
    pub fn kind(&self) -> &'static str {
        match self {
            VoiceInputSource::File { .. } => "file",
            VoiceInputSource::Microphone => "microphone",
            VoiceInputSource::Matrix => "matrix",
            VoiceInputSource::Remote { .. } => "remote",
        }
    }

    pub fn label(&self) -> String {
        match self {
            VoiceInputSource::File { extension } => extension
                .as_ref()
                .map(|extension| format!("file:{extension}"))
                .unwrap_or_else(|| "file".to_string()),
            VoiceInputSource::Microphone => "microphone".to_string(),
            VoiceInputSource::Matrix => "matrix".to_string(),
            VoiceInputSource::Remote { kind } => kind.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VoiceReplyMode {
    Text,
    Tts,
    None,
}

impl VoiceReplyMode {
    pub fn as_str(self) -> &'static str {
        match self {
            VoiceReplyMode::Text => "text",
            VoiceReplyMode::Tts => "tts",
            VoiceReplyMode::None => "none",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceValidation {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
    pub input_kind: String,
    pub input_label: String,
    pub reply_mode: &'static str,
    pub supported: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

pub fn parse_input_source(input: &str) -> VoiceInputSource {
    let normalized = input.trim();
    let lower = normalized.to_ascii_lowercase();
    match lower.as_str() {
        "microphone" | "mic" => VoiceInputSource::Microphone,
        "matrix" => VoiceInputSource::Matrix,
        _ if lower.starts_with("http://") => VoiceInputSource::Remote {
            kind: "http".to_string(),
        },
        _ if lower.starts_with("https://") => VoiceInputSource::Remote {
            kind: "https".to_string(),
        },
        _ if lower.starts_with("remote:") => VoiceInputSource::Remote {
            kind: "remote".to_string(),
        },
        _ if lower.starts_with("cloud:") || lower.starts_with("s3://") => VoiceInputSource::Remote {
            kind: "cloud".to_string(),
        },
        _ => {
            let path = normalized.strip_prefix("file:").unwrap_or(normalized);
            let extension = Path::new(path)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_ascii_lowercase())
                .filter(|value| !value.is_empty());
            VoiceInputSource::File { extension }
        }
    }
}

pub fn parse_reply_mode(input: Option<&str>) -> Result<VoiceReplyMode, String> {
    let Some(input) = input else {
        return Ok(VoiceReplyMode::Text);
    };
    match input.trim().to_ascii_lowercase().as_str() {
        "" | "text" => Ok(VoiceReplyMode::Text),
        "tts" | "speech" => Ok(VoiceReplyMode::Tts),
        "none" | "off" => Ok(VoiceReplyMode::None),
        other => Err(format!("unknown voice reply mode '{other}'")),
    }
}

pub fn validate(source: &VoiceInputSource, reply_mode: VoiceReplyMode, matrix_active: bool) -> VoiceValidation {
    let mut validation = VoiceValidation {
        source: "voice_mode",
        action: "validate",
        status: "success",
        backend: "local-policy",
        input_kind: source.kind().to_string(),
        input_label: source.label(),
        reply_mode: reply_mode.as_str(),
        supported: true,
        error_kind: None,
        error_message: None,
    };

    match source {
        VoiceInputSource::File { .. } => validation,
        VoiceInputSource::Matrix if matrix_active => {
            validation.backend = "matrix-existing-bridge";
            validation
        }
        VoiceInputSource::Microphone => unsupported(
            validation,
            "unsupported_input",
            "live microphone capture is not supported in the first-pass voice mode",
        ),
        VoiceInputSource::Matrix => unsupported(
            validation,
            "unsupported_input",
            "matrix voice transcription requires an active Matrix bridge implementation",
        ),
        VoiceInputSource::Remote { kind } => unsupported(
            validation,
            "unsupported_input",
            &format!("remote audio input '{kind}' is not supported in the first-pass voice mode"),
        ),
    }
}

pub fn status_summary() -> VoiceValidation {
    validate(&VoiceInputSource::File { extension: None }, VoiceReplyMode::Text, false)
}

fn unsupported(mut validation: VoiceValidation, kind: &'static str, message: &str) -> VoiceValidation {
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
    fn parses_file_inputs_without_preserving_full_paths() {
        assert_eq!(parse_input_source("voice.wav"), VoiceInputSource::File {
            extension: Some("wav".to_string())
        });
        assert_eq!(parse_input_source("file:/tmp/private/audio.MP3").label(), "file:mp3");
    }

    #[test]
    fn parses_remote_inputs_as_safe_kinds() {
        assert_eq!(parse_input_source("https://token@example.test/audio.wav").label(), "https");
        assert_eq!(parse_input_source("s3://bucket/key.wav").label(), "cloud");
    }

    #[test]
    fn parses_reply_modes_and_rejects_unknown_modes() {
        assert_eq!(parse_reply_mode(None).unwrap(), VoiceReplyMode::Text);
        assert_eq!(parse_reply_mode(Some("tts")).unwrap(), VoiceReplyMode::Tts);
        assert_eq!(parse_reply_mode(Some("off")).unwrap(), VoiceReplyMode::None);
        assert!(parse_reply_mode(Some("broadcast")).unwrap_err().contains("unknown"));
    }

    #[test]
    fn validates_file_and_rejects_microphone_first_pass() {
        let file = validate(&parse_input_source("audio.wav"), VoiceReplyMode::Text, false);
        assert!(file.supported);
        assert_eq!(file.input_label, "file:wav");

        let mic = validate(&parse_input_source("microphone"), VoiceReplyMode::Text, false);
        assert!(!mic.supported);
        assert_eq!(mic.error_kind, Some("unsupported_input"));
    }

    #[test]
    fn remote_errors_are_replay_safe() {
        let remote =
            validate(&parse_input_source("https://token@example.test/audio.wav\nsecret"), VoiceReplyMode::Tts, false);
        assert!(!remote.supported);
        assert_eq!(remote.input_label, "https");
        let message = remote.error_message.expect("message");
        assert!(!message.contains('\n'));
        assert!(!message.contains("token@example.test"));
    }
}
