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
