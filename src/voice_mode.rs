//! Voice and speech-to-text first-pass policy helpers.
//!
//! This module validates voice/STT intent without capturing microphone input,
//! reading raw audio, or contacting transcription providers.

use std::path::Path;

use serde::Deserialize;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SttProviderPolicy {
    LocalFake,
    CloudDisabled,
}

impl SttProviderPolicy {
    pub fn as_str(self) -> &'static str {
        match self {
            SttProviderPolicy::LocalFake => "local-fake",
            SttProviderPolicy::CloudDisabled => "cloud-disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VoiceCapturePolicy {
    pub enabled: bool,
    pub provider: SttProviderPolicy,
    pub retain_audio: bool,
    pub auto_submit: bool,
}

impl Default for VoiceCapturePolicy {
    fn default() -> Self {
        Self {
            enabled: false,
            provider: SttProviderPolicy::LocalFake,
            retain_audio: false,
            auto_submit: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceCaptureRequest {
    pub session_id: Option<String>,
    pub source: VoiceInputSource,
    pub reply_mode: VoiceReplyMode,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceCaptureReceipt {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub backend: &'static str,
    pub input_kind: String,
    pub input_label: String,
    pub reply_mode: &'static str,
    pub capture_active: bool,
    pub raw_audio_retained: bool,
    pub auto_submit: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_request: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_kind: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VoiceSessionPrompt {
    pub source: &'static str,
    pub action: &'static str,
    pub status: &'static str,
    pub prompt: String,
    pub reply_mode: &'static str,
    pub auto_submit: bool,
    pub transcript_chars: usize,
    pub transcript_digest: String,
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

pub fn start_capture(policy: &VoiceCapturePolicy, request: VoiceCaptureRequest) -> VoiceCaptureReceipt {
    let mut receipt = base_capture_receipt("start_capture", policy, &request, false);
    if !policy.enabled {
        return unsupported_capture(
            receipt,
            "voice_disabled",
            "live voice capture is disabled; pass explicit enablement before opening a capture stream",
        );
    }
    if matches!(policy.provider, SttProviderPolicy::CloudDisabled) {
        return unsupported_capture(
            receipt,
            "provider_disabled",
            "cloud speech-to-text providers are disabled by policy",
        );
    }
    if !matches!(request.source, VoiceInputSource::Microphone | VoiceInputSource::File { .. }) {
        return unsupported_capture(
            receipt,
            "unsupported_input",
            "live capture supports only microphone or local file sources",
        );
    }
    receipt.status = "active";
    receipt.capture_active = true;
    receipt.provider_request = Some("open");
    receipt
}

pub fn stop_capture(policy: &VoiceCapturePolicy, request: VoiceCaptureRequest) -> VoiceCaptureReceipt {
    let mut receipt = base_capture_receipt("stop_capture", policy, &request, false);
    receipt.status = "stopped";
    receipt.provider_request = Some("closed");
    receipt
}

pub fn session_prompt_from_transcript(
    transcript: &str,
    reply_mode: VoiceReplyMode,
    auto_submit: bool,
) -> Result<VoiceSessionPrompt, String> {
    let normalized = transcript.trim();
    if normalized.is_empty() {
        return Err("voice transcript is empty".to_string());
    }
    if normalized.len() > 16 * 1024 {
        return Err("voice transcript exceeds prompt handoff limit".to_string());
    }
    Ok(VoiceSessionPrompt {
        source: "voice_mode",
        action: "submit_transcript",
        status: if auto_submit { "submitted" } else { "prepared" },
        prompt: normalized.to_string(),
        reply_mode: reply_mode.as_str(),
        auto_submit,
        transcript_chars: normalized.chars().count(),
        transcript_digest: transcript_digest(normalized),
    })
}

fn base_capture_receipt(
    action: &'static str,
    policy: &VoiceCapturePolicy,
    request: &VoiceCaptureRequest,
    active: bool,
) -> VoiceCaptureReceipt {
    VoiceCaptureReceipt {
        source: "voice_mode",
        action,
        status: "success",
        backend: policy.provider.as_str(),
        input_kind: request.source.kind().to_string(),
        input_label: request.source.label(),
        reply_mode: request.reply_mode.as_str(),
        capture_active: active,
        raw_audio_retained: policy.retain_audio,
        auto_submit: policy.auto_submit,
        session_id: request.session_id.clone().map(|value| sanitize_error_message(&value)),
        provider_request: None,
        error_kind: None,
        error_message: None,
    }
}

fn unsupported_capture(mut receipt: VoiceCaptureReceipt, kind: &'static str, message: &str) -> VoiceCaptureReceipt {
    receipt.status = "unsupported";
    receipt.capture_active = false;
    receipt.error_kind = Some(kind);
    receipt.error_message = Some(sanitize_error_message(message));
    receipt
}

fn transcript_digest(transcript: &str) -> String {
    let mut state: u64 = 0xcbf29ce484222325;
    for byte in transcript.as_bytes() {
        state ^= u64::from(*byte);
        state = state.wrapping_mul(0x100000001b3);
    }
    format!("fnv64:{state:016x}")
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

    #[test]
    fn live_capture_requires_explicit_enablement_and_returns_safe_receipts() {
        let request = VoiceCaptureRequest {
            session_id: Some("session\nsecret".to_string()),
            source: VoiceInputSource::Microphone,
            reply_mode: VoiceReplyMode::Text,
        };
        let disabled = start_capture(&VoiceCapturePolicy::default(), request.clone());
        assert_eq!(disabled.status, "unsupported");
        assert_eq!(disabled.error_kind, Some("voice_disabled"));
        assert!(!disabled.capture_active);
        assert!(!disabled.session_id.expect("session id").contains('\n'));

        let policy = VoiceCapturePolicy {
            enabled: true,
            provider: SttProviderPolicy::LocalFake,
            retain_audio: false,
            auto_submit: true,
        };
        let active = start_capture(&policy, request.clone());
        assert_eq!(active.status, "active");
        assert!(active.capture_active);
        assert_eq!(active.provider_request, Some("open"));
        assert!(!active.raw_audio_retained);

        let stopped = stop_capture(&policy, request);
        assert_eq!(stopped.status, "stopped");
        assert!(!stopped.capture_active);
        assert_eq!(stopped.provider_request, Some("closed"));
    }

    #[test]
    fn transcript_prompt_flow_preserves_prompt_but_receipts_use_digest() {
        let prompt = session_prompt_from_transcript("  hello from voice  ", VoiceReplyMode::Tts, false).unwrap();
        assert_eq!(prompt.prompt, "hello from voice");
        assert_eq!(prompt.status, "prepared");
        assert_eq!(prompt.reply_mode, "tts");
        assert_eq!(prompt.transcript_chars, 16);
        assert!(prompt.transcript_digest.starts_with("fnv64:"));
        assert!(session_prompt_from_transcript("   ", VoiceReplyMode::Text, false).is_err());
    }
}
