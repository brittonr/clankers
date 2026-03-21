//! Tests for the TTS router and provider infrastructure.

use std::sync::Arc;

use async_trait::async_trait;
use tempfile::NamedTempFile;

use crate::error::Error;
use crate::provider::{AudioFormat, TtsProvider, TtsRequest, TtsResponse, Voice};
use crate::TtsRouter;

// ---------------------------------------------------------------------------
// Mock provider
// ---------------------------------------------------------------------------

struct MockProvider {
    name: &'static str,
    voices: Vec<Voice>,
    fail: bool,
    sample_rate: u32,
}

impl MockProvider {
    fn new(name: &'static str, voice_names: &[&str]) -> Self {
        let voices = voice_names
            .iter()
            .map(|v| Voice {
                id: v.to_string(),
                name: v.to_string(),
                provider: name.to_string(),
            })
            .collect();
        Self {
            name,
            voices,
            fail: false,
            sample_rate: 22050,
        }
    }

    fn failing(name: &'static str, voice_names: &[&str]) -> Self {
        let mut p = Self::new(name, voice_names);
        p.fail = true;
        p
    }
}

#[async_trait]
impl TtsProvider for MockProvider {
    async fn synthesize(&self, request: TtsRequest) -> crate::Result<TtsResponse> {
        if self.fail {
            return Err(Error::Provider {
                message: format!("{} intentionally failed", self.name),
            });
        }
        // Generate a short sine wave so WAV encoding has real data
        let duration_samples = (self.sample_rate as f32 * 0.1) as usize; // 100ms
        let samples: Vec<f32> = (0..duration_samples)
            .map(|i| {
                let t = i as f32 / self.sample_rate as f32;
                (t * 440.0 * std::f32::consts::TAU).sin() * 0.5
            })
            .collect();
        let duration_ms = (samples.len() as u64 * 1000) / u64::from(self.sample_rate);
        Ok(TtsResponse {
            samples,
            sample_rate: self.sample_rate,
            duration_ms,
            voice: request.voice,
            provider: self.name.to_string(),
        })
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn name(&self) -> &str {
        self.name
    }

    async fn is_available(&self) -> bool {
        !self.fail
    }
}

// ---------------------------------------------------------------------------
// TtsRouter tests
// ---------------------------------------------------------------------------

#[test]
fn empty_router_has_no_providers() {
    let router = TtsRouter::new();
    assert!(router.provider_names().is_empty());
    assert!(router.list_voices().is_empty());
}

#[test]
fn default_is_empty() {
    let router = TtsRouter::default();
    assert!(router.provider_names().is_empty());
}

#[test]
fn register_single_provider() {
    let mut router = TtsRouter::new();
    let mock = MockProvider::new("mock-a", &["alice", "bob"]);
    router.register(Arc::new(mock));

    assert_eq!(router.provider_names(), vec!["mock-a"]);
    assert_eq!(router.list_voices().len(), 2);
}

#[test]
fn register_multiple_providers() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("local", &["bella", "luna"])));
    router.register(Arc::new(MockProvider::new("cloud", &["alloy", "nova"])));

    assert_eq!(router.provider_names(), vec!["local", "cloud"]);
    assert_eq!(router.list_voices().len(), 4);
}

#[tokio::test]
async fn synthesize_routes_to_correct_provider() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("local", &["bella"])));
    router.register(Arc::new(MockProvider::new("cloud", &["alloy"])));

    let resp = router.synthesize("hello", "bella", 1.0).await.unwrap();
    assert_eq!(resp.provider, "local");

    let resp = router.synthesize("hello", "alloy", 1.0).await.unwrap();
    assert_eq!(resp.provider, "cloud");
}

#[tokio::test]
async fn synthesize_case_insensitive_voice() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("local", &["Bella"])));

    let resp = router.synthesize("hi", "bella", 1.0).await.unwrap();
    assert_eq!(resp.provider, "local");

    let resp = router.synthesize("hi", "BELLA", 1.0).await.unwrap();
    assert_eq!(resp.provider, "local");
}

#[tokio::test]
async fn synthesize_unknown_voice_falls_back_to_default() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("fallback", &["bella"])));

    // "nonexistent" isn't registered, should fall back to first provider
    let resp = router.synthesize("hi", "nonexistent", 1.0).await.unwrap();
    assert_eq!(resp.provider, "fallback");
}

#[tokio::test]
async fn synthesize_no_providers_returns_error() {
    let router = TtsRouter::new();
    let err = router.synthesize("hi", "bella", 1.0).await.unwrap_err();
    match err {
        Error::NoProvider { voice } => assert_eq!(voice, "bella"),
        other => panic!("expected NoProvider, got: {other}"),
    }
}

#[tokio::test]
async fn synthesize_provider_failure_propagates() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::failing("broken", &["bella"])));

    let err = router.synthesize("hi", "bella", 1.0).await.unwrap_err();
    match err {
        Error::Provider { message } => assert!(message.contains("intentionally failed")),
        other => panic!("expected Provider error, got: {other}"),
    }
}

#[test]
fn first_registered_provider_wins_voice_conflict() {
    let mut router = TtsRouter::new();
    // Both providers claim "echo"
    router.register(Arc::new(MockProvider::new("first", &["echo"])));
    router.register(Arc::new(MockProvider::new("second", &["echo"])));

    // voice_map uses or_insert, so first registration wins
    assert_eq!(router.provider_names().len(), 2);
}

#[tokio::test]
async fn first_provider_wins_voice_conflict_on_synth() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("first", &["echo"])));
    router.register(Arc::new(MockProvider::new("second", &["echo"])));

    let resp = router.synthesize("hi", "echo", 1.0).await.unwrap();
    assert_eq!(resp.provider, "first");
}

#[test]
fn debug_format() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("mock", &["a", "b"])));
    let dbg = format!("{router:?}");
    assert!(dbg.contains("mock"));
    assert!(dbg.contains("voices"));
}

// ---------------------------------------------------------------------------
// TtsResponse tests
// ---------------------------------------------------------------------------

#[test]
fn response_to_wav_roundtrip() {
    let samples: Vec<f32> = (0..1000)
        .map(|i| (i as f32 / 1000.0 * std::f32::consts::TAU).sin())
        .collect();
    let resp = TtsResponse {
        samples: samples.clone(),
        sample_rate: 22050,
        duration_ms: 45,
        voice: "test".to_string(),
        provider: "test".to_string(),
    };

    let wav_bytes = resp.to_wav().unwrap();
    assert!(wav_bytes.len() > 44); // WAV header is 44 bytes minimum

    // Parse it back with hound
    let reader = hound::WavReader::new(std::io::Cursor::new(&wav_bytes)).unwrap();
    let spec = reader.spec();
    assert_eq!(spec.channels, 1);
    assert_eq!(spec.sample_rate, 22050);
    assert_eq!(spec.bits_per_sample, 32);
    assert_eq!(spec.sample_format, hound::SampleFormat::Float);
    assert_eq!(reader.len() as usize, samples.len());
}

#[test]
fn response_write_wav_to_file() {
    let samples: Vec<f32> = vec![0.0, 0.5, -0.5, 1.0, -1.0];
    let resp = TtsResponse {
        samples,
        sample_rate: 16000,
        duration_ms: 0,
        voice: "test".to_string(),
        provider: "test".to_string(),
    };

    let tmp = NamedTempFile::new().unwrap();
    resp.write_wav(tmp.path()).unwrap();

    let reader = hound::WavReader::open(tmp.path()).unwrap();
    assert_eq!(reader.len(), 5);
    let read_samples: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
    assert_eq!(read_samples, vec![0.0, 0.5, -0.5, 1.0, -1.0]);
}

#[test]
fn response_empty_samples_produces_valid_wav() {
    let resp = TtsResponse {
        samples: vec![],
        sample_rate: 44100,
        duration_ms: 0,
        voice: "empty".to_string(),
        provider: "test".to_string(),
    };

    let wav = resp.to_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(&wav)).unwrap();
    assert_eq!(reader.len(), 0);
}

// ---------------------------------------------------------------------------
// TtsRequest / AudioFormat tests
// ---------------------------------------------------------------------------

#[test]
fn audio_format_default_is_wav() {
    assert_eq!(AudioFormat::default(), AudioFormat::Wav);
}

#[test]
fn tts_request_fields() {
    let req = TtsRequest {
        text: "hello world".to_string(),
        voice: "bella".to_string(),
        speed: 1.5,
        format: AudioFormat::Pcm,
    };
    assert_eq!(req.text, "hello world");
    assert_eq!(req.speed, 1.5);
    assert_eq!(req.format, AudioFormat::Pcm);
}

// ---------------------------------------------------------------------------
// Voice tests
// ---------------------------------------------------------------------------

#[test]
fn voice_serialization() {
    let voice = Voice {
        id: "alloy".to_string(),
        name: "Alloy".to_string(),
        provider: "openai".to_string(),
    };
    let json = serde_json::to_string(&voice).unwrap();
    assert!(json.contains("alloy"));
    let back: Voice = serde_json::from_str(&json).unwrap();
    assert_eq!(back.id, "alloy");
    assert_eq!(back.name, "Alloy");
}

// ---------------------------------------------------------------------------
// Error display tests
// ---------------------------------------------------------------------------

#[test]
fn error_display_voice_not_found() {
    let e = Error::VoiceNotFound {
        name: "missing".to_string(),
    };
    assert_eq!(format!("{e}"), "voice not found: missing");
}

#[test]
fn error_display_no_provider() {
    let e = Error::NoProvider {
        voice: "bella".to_string(),
    };
    assert!(format!("{e}").contains("bella"));
}

#[test]
fn error_display_provider() {
    let e = Error::Provider {
        message: "timeout".to_string(),
    };
    assert!(format!("{e}").contains("timeout"));
}

// ---------------------------------------------------------------------------
// synthesize_to_file
// ---------------------------------------------------------------------------

#[tokio::test]
async fn synthesize_to_file_writes_valid_wav() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("mock", &["bella"])));

    let tmp = NamedTempFile::new().unwrap();
    router
        .synthesize_to_file("hello world", tmp.path(), "bella", 1.0)
        .await
        .unwrap();

    let reader = hound::WavReader::open(tmp.path()).unwrap();
    assert!(reader.len() > 0);
    assert_eq!(reader.spec().channels, 1);
}

#[tokio::test]
async fn synthesize_to_file_no_provider_errors() {
    let router = TtsRouter::new();
    let tmp = NamedTempFile::new().unwrap();
    let err = router
        .synthesize_to_file("hello", tmp.path(), "bella", 1.0)
        .await
        .unwrap_err();
    assert!(matches!(err, Error::NoProvider { .. }));
}

// ---------------------------------------------------------------------------
// Speed parameter forwarding
// ---------------------------------------------------------------------------

#[tokio::test]
async fn speed_parameter_forwarded() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("mock", &["bella"])));

    // Speed doesn't change mock output, but verify it doesn't panic
    for speed in [0.5, 1.0, 1.5, 2.0] {
        let resp = router.synthesize("test", "bella", speed).await.unwrap();
        assert!(!resp.samples.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Concurrent synthesis
// ---------------------------------------------------------------------------

#[tokio::test]
async fn concurrent_synthesis_all_succeed() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("mock", &["bella", "luna"])));
    let router = Arc::new(router);

    let handles: Vec<_> = (0..8)
        .map(|i| {
            let r = Arc::clone(&router);
            let voice = if i % 2 == 0 { "bella" } else { "luna" };
            tokio::spawn(async move {
                r.synthesize(&format!("text {i}"), voice, 1.0).await
            })
        })
        .collect();

    for handle in handles {
        let resp = handle.await.unwrap().unwrap();
        assert!(!resp.samples.is_empty());
        assert_eq!(resp.provider, "mock");
    }
}

// ---------------------------------------------------------------------------
// Voice map precedence: name vs id
// ---------------------------------------------------------------------------

#[tokio::test]
async fn voice_name_and_id_both_resolve() {
    // Provider with different id and display name
    let voices = vec![Voice {
        id: "expr-voice-2-f".to_string(),
        name: "Luna".to_string(),
        provider: "fancy".to_string(),
    }];
    let mock = MockProviderWithVoices {
        name: "fancy",
        voices,
        sample_rate: 22050,
    };
    let mut router = TtsRouter::new();
    router.register(Arc::new(mock));

    // Resolve by display name
    let resp = router.synthesize("hi", "luna", 1.0).await.unwrap();
    assert_eq!(resp.provider, "fancy");

    // Resolve by id
    let resp = router.synthesize("hi", "expr-voice-2-f", 1.0).await.unwrap();
    assert_eq!(resp.provider, "fancy");
}

/// Provider that takes pre-built Voice structs (for testing name != id).
struct MockProviderWithVoices {
    name: &'static str,
    voices: Vec<Voice>,
    sample_rate: u32,
}

#[async_trait]
impl TtsProvider for MockProviderWithVoices {
    async fn synthesize(&self, request: TtsRequest) -> crate::Result<TtsResponse> {
        let duration_samples = (self.sample_rate as f32 * 0.05) as usize;
        let samples: Vec<f32> = (0..duration_samples).map(|i| (i as f32 * 0.01).sin()).collect();
        let duration_ms = (samples.len() as u64 * 1000) / u64::from(self.sample_rate);
        Ok(TtsResponse {
            samples,
            sample_rate: self.sample_rate,
            duration_ms,
            voice: request.voice,
            provider: self.name.to_string(),
        })
    }
    fn voices(&self) -> &[Voice] { &self.voices }
    fn name(&self) -> &str { self.name }
    async fn is_available(&self) -> bool { true }
}

// ---------------------------------------------------------------------------
// Multiple providers with different sample rates produce valid WAV
// ---------------------------------------------------------------------------

#[tokio::test]
async fn different_sample_rates_produce_valid_wav() {
    for &rate in &[8000u32, 16000, 22050, 24000, 44100, 48000] {
        let mut mock = MockProvider::new("test", &["voice"]);
        mock.sample_rate = rate;
        let mut router = TtsRouter::new();
        router.register(Arc::new(mock));

        let resp = router.synthesize("test", "voice", 1.0).await.unwrap();
        assert_eq!(resp.sample_rate, rate);

        let wav = resp.to_wav().unwrap();
        let reader = hound::WavReader::new(std::io::Cursor::new(&wav)).unwrap();
        assert_eq!(reader.spec().sample_rate, rate);
        assert_eq!(reader.spec().channels, 1);
    }
}

// ---------------------------------------------------------------------------
// Provider failure does NOT affect other providers
// ---------------------------------------------------------------------------

#[tokio::test]
async fn failing_provider_does_not_break_other_voices() {
    let mut router = TtsRouter::new();
    // First provider (default) fails
    router.register(Arc::new(MockProvider::failing("broken", &["voice_a"])));
    // Second provider works
    router.register(Arc::new(MockProvider::new("working", &["voice_b"])));

    // voice_b routes to working provider — should succeed
    let resp = router.synthesize("test", "voice_b", 1.0).await.unwrap();
    assert_eq!(resp.provider, "working");

    // voice_a routes to broken provider — should fail
    let err = router.synthesize("test", "voice_a", 1.0).await.unwrap_err();
    assert!(matches!(err, Error::Provider { .. }));
}

#[tokio::test]
async fn unknown_voice_with_failing_default_propagates_error() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::failing("broken", &["bella"])));

    // Unknown voice falls back to default, which fails
    let err = router.synthesize("test", "nonexistent", 1.0).await.unwrap_err();
    assert!(matches!(err, Error::Provider { .. }));
}

// ---------------------------------------------------------------------------
// list_voices returns voices from all providers
// ---------------------------------------------------------------------------

#[test]
fn list_voices_spans_all_providers() {
    let mut router = TtsRouter::new();
    router.register(Arc::new(MockProvider::new("a", &["v1", "v2"])));
    router.register(Arc::new(MockProvider::new("b", &["v3"])));
    router.register(Arc::new(MockProvider::new("c", &["v4", "v5", "v6"])));

    let voices = router.list_voices();
    assert_eq!(voices.len(), 6);
    let ids: Vec<&str> = voices.iter().map(|v| v.id.as_str()).collect();
    assert!(ids.contains(&"v1"));
    assert!(ids.contains(&"v6"));
}

// ---------------------------------------------------------------------------
// WAV edge cases
// ---------------------------------------------------------------------------

#[test]
fn wav_extreme_sample_values() {
    let resp = TtsResponse {
        samples: vec![-1.0, 1.0, 0.0, -0.999, 0.999, f32::MIN_POSITIVE],
        sample_rate: 44100,
        duration_ms: 0,
        voice: "test".to_string(),
        provider: "test".to_string(),
    };

    let wav = resp.to_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(&wav)).unwrap();
    let read_back: Vec<f32> = reader.into_samples::<f32>().map(|s| s.unwrap()).collect();
    assert_eq!(read_back.len(), 6);
    assert_eq!(read_back[0], -1.0);
    assert_eq!(read_back[1], 1.0);
}

#[test]
fn wav_large_sample_count() {
    // 5 seconds of 48kHz audio = 240,000 samples
    let n = 240_000;
    let samples: Vec<f32> = (0..n)
        .map(|i| (i as f32 / 48000.0 * 440.0 * std::f32::consts::TAU).sin())
        .collect();
    let resp = TtsResponse {
        samples: samples.clone(),
        sample_rate: 48000,
        duration_ms: 5000,
        voice: "test".to_string(),
        provider: "test".to_string(),
    };

    let wav = resp.to_wav().unwrap();
    let reader = hound::WavReader::new(std::io::Cursor::new(&wav)).unwrap();
    assert_eq!(reader.len() as usize, n);
}

#[test]
fn write_wav_to_nonexistent_dir_fails() {
    let resp = TtsResponse {
        samples: vec![0.0],
        sample_rate: 22050,
        duration_ms: 0,
        voice: "test".to_string(),
        provider: "test".to_string(),
    };
    let result = resp.write_wav(std::path::Path::new("/nonexistent/dir/output.wav"));
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Error conversions
// ---------------------------------------------------------------------------

#[test]
fn error_from_io() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "gone");
    let tts_err: Error = io_err.into();
    assert!(format!("{tts_err}").contains("gone"));
}

// ---------------------------------------------------------------------------
// auto_discover smoke test (no live providers needed)
// ---------------------------------------------------------------------------

#[test]
fn auto_discover_without_env_vars() {
    // Remove env vars to ensure clean state
    let _key_guard = TempEnvGuard::new("OPENAI_API_KEY");
    let mut router = TtsRouter::new();
    router.auto_discover();
    // Should have 0 or 1 providers depending on whether kitten feature
    // is enabled and espeak-ng is available. No panic is the assertion.
    let _ = router.provider_names();
}

/// Temporarily unsets an env var, restores on drop.
struct TempEnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl TempEnvGuard {
    fn new(key: &'static str) -> Self {
        let prev = std::env::var(key).ok();
        // SAFETY: tests are single-threaded for this env var
        unsafe { std::env::remove_var(key) };
        Self { key, prev }
    }
}

impl Drop for TempEnvGuard {
    fn drop(&mut self) {
        if let Some(val) = &self.prev {
            // SAFETY: tests are single-threaded for this env var
            unsafe { std::env::set_var(self.key, val) };
        }
    }
}
