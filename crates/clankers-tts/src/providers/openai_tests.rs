//! Tests for the OpenAI TTS provider.

use super::*;

// ---------------------------------------------------------------------------
// Construction
// ---------------------------------------------------------------------------

#[test]
fn new_creates_provider_with_correct_voices() {
    let p = OpenAiTtsProvider::new("test-key".to_string(), OpenAiModel::default());
    let voices = p.voices();
    assert_eq!(voices.len(), 10);

    let ids: Vec<&str> = voices.iter().map(|v| v.id.as_str()).collect();
    assert!(ids.contains(&"alloy"));
    assert!(ids.contains(&"shimmer"));
    assert!(ids.contains(&"nova"));
    assert!(ids.contains(&"echo"));

    // All voices should report openai-tts provider
    for v in voices {
        assert_eq!(v.provider, "openai-tts");
    }
}

#[test]
fn name_is_openai_tts() {
    let p = OpenAiTtsProvider::new("k".to_string(), OpenAiModel::Tts1);
    assert_eq!(p.name(), "openai-tts");
}

// ---------------------------------------------------------------------------
// Model variants
// ---------------------------------------------------------------------------

#[test]
fn model_as_str() {
    assert_eq!(OpenAiModel::Tts1.as_str(), "tts-1");
    assert_eq!(OpenAiModel::Tts1Hd.as_str(), "tts-1-hd");
    assert_eq!(OpenAiModel::Gpt4oMiniTts.as_str(), "gpt-4o-mini-tts");
}

#[test]
fn default_model_is_tts1() {
    let m = OpenAiModel::default();
    assert_eq!(m.as_str(), "tts-1");
}

// ---------------------------------------------------------------------------
// from_env
// ---------------------------------------------------------------------------

#[test]
fn from_env_missing_key_returns_none() {
    // Temporarily unset the key
    let prev = std::env::var("OPENAI_API_KEY").ok();
    unsafe { std::env::remove_var("OPENAI_API_KEY") };

    let result = OpenAiTtsProvider::from_env();
    assert!(result.is_none());

    // Restore
    if let Some(val) = prev {
        unsafe { std::env::set_var("OPENAI_API_KEY", val) };
    }
}

#[test]
fn from_env_empty_key_returns_none() {
    let prev = std::env::var("OPENAI_API_KEY").ok();
    unsafe { std::env::set_var("OPENAI_API_KEY", "") };

    let result = OpenAiTtsProvider::from_env();
    assert!(result.is_none());

    // Restore
    if let Some(val) = prev {
        unsafe { std::env::set_var("OPENAI_API_KEY", val) };
    } else {
        unsafe { std::env::remove_var("OPENAI_API_KEY") };
    }
}

// ---------------------------------------------------------------------------
// is_available
// ---------------------------------------------------------------------------

#[tokio::test]
async fn is_available_with_key() {
    let p = OpenAiTtsProvider::new("real-key".to_string(), OpenAiModel::Tts1);
    assert!(p.is_available().await);
}

#[tokio::test]
async fn is_available_empty_key() {
    let p = OpenAiTtsProvider::new(String::new(), OpenAiModel::Tts1);
    assert!(!p.is_available().await);
}

// ---------------------------------------------------------------------------
// Debug impl
// ---------------------------------------------------------------------------

#[test]
fn debug_does_not_leak_api_key() {
    let p = OpenAiTtsProvider::new("sk-super-secret-key".to_string(), OpenAiModel::Tts1Hd);
    let dbg = format!("{p:?}");
    assert!(!dbg.contains("sk-super-secret-key"), "Debug should not leak API key");
    assert!(dbg.contains("has_key"));
    assert!(dbg.contains("Tts1Hd"));
}

// ---------------------------------------------------------------------------
// PCM decode logic (unit test the conversion independently)
// ---------------------------------------------------------------------------

#[test]
fn pcm16_to_f32_conversion_range() {
    // i16::MAX → ~1.0, i16::MIN → -1.0, 0 → 0.0
    let cases: Vec<(i16, f32)> = vec![
        (0, 0.0),
        (i16::MAX, i16::MAX as f32 / 32768.0),
        (i16::MIN, -1.0),
        (16384, 0.5),
        (-16384, -0.5),
    ];

    for (input, expected) in cases {
        let bytes = input.to_le_bytes();
        let sample = i16::from_le_bytes([bytes[0], bytes[1]]);
        let result = f32::from(sample) / 32768.0;
        assert!(
            (result - expected).abs() < 0.001,
            "pcm16({input}) = {result}, expected {expected}"
        );
    }
}

#[test]
fn pcm16_odd_byte_count_truncates() {
    // If OpenAI returns odd bytes, chunks_exact(2) should skip the last byte
    let bytes: &[u8] = &[0, 0, 1, 0, 99]; // 5 bytes = 2 complete samples + 1 leftover
    let samples: Vec<f32> = bytes
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            f32::from(sample) / 32768.0
        })
        .collect();
    assert_eq!(samples.len(), 2);
}

// ---------------------------------------------------------------------------
// HTTP error path (invalid API key)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn synthesize_with_invalid_key_returns_error() {
    let p = OpenAiTtsProvider::new("invalid-key-12345".to_string(), OpenAiModel::Tts1);
    let request = crate::provider::TtsRequest {
        text: "test".to_string(),
        voice: "alloy".to_string(),
        speed: 1.0,
        format: crate::provider::AudioFormat::default(),
    };

    let result = p.synthesize(request).await;
    // Should fail with either HTTP error or Provider error
    assert!(result.is_err());
    let err = result.unwrap_err();
    let msg = format!("{err}");
    // Either a connection error or an auth error from the API
    assert!(
        msg.contains("401") || msg.contains("error") || msg.contains("HTTP"),
        "Expected auth/connection error, got: {msg}"
    );
}

// ---------------------------------------------------------------------------
// Voice ID lookup
// ---------------------------------------------------------------------------

#[test]
fn all_voice_ids_are_lowercase() {
    let p = OpenAiTtsProvider::new("k".to_string(), OpenAiModel::Tts1);
    for voice in p.voices() {
        assert_eq!(
            voice.id,
            voice.id.to_lowercase(),
            "voice id should be lowercase: {}",
            voice.id
        );
    }
}

#[test]
fn voice_name_matches_id() {
    // OpenAI voices have name == id
    let p = OpenAiTtsProvider::new("k".to_string(), OpenAiModel::Tts1);
    for voice in p.voices() {
        assert_eq!(voice.name, voice.id);
    }
}
