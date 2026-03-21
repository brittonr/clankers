//! TTS provider trait and request/response types.

use std::path::Path;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// A single voice offered by a provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Voice {
    /// Internal ID used in requests (e.g. "alloy", "Bella", "expr-voice-2-f").
    pub id: String,
    /// Human-friendly display name.
    pub name: String,
    /// Provider that owns this voice.
    pub provider: String,
}

/// Request to synthesize speech.
#[derive(Debug, Clone)]
pub struct TtsRequest {
    /// Text to synthesize.
    pub text: String,
    /// Voice ID or name.
    pub voice: String,
    /// Speech speed multiplier (1.0 = normal).
    pub speed: f32,
    /// Output format hint ("wav", "mp3", "opus", "pcm").
    pub format: AudioFormat,
}

/// Audio output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AudioFormat {
    #[default]
    Wav,
    Pcm,
}

/// Response from a TTS provider.
pub struct TtsResponse {
    /// Raw float32 PCM audio samples.
    pub samples: Vec<f32>,
    /// Sample rate in Hz.
    pub sample_rate: u32,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Which voice was actually used (after alias resolution).
    pub voice: String,
    /// Which provider served the request.
    pub provider: String,
}

impl TtsResponse {
    /// Encode as a WAV byte buffer.
    pub fn to_wav(&self) -> Result<Vec<u8>, hound::Error> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut cursor = std::io::Cursor::new(Vec::new());
        {
            let mut writer = hound::WavWriter::new(&mut cursor, spec)?;
            for &sample in &self.samples {
                writer.write_sample(sample)?;
            }
            writer.finalize()?;
        }
        Ok(cursor.into_inner())
    }

    /// Write to a WAV file.
    pub fn write_wav(&self, path: &Path) -> Result<(), hound::Error> {
        let spec = hound::WavSpec {
            channels: 1,
            sample_rate: self.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = hound::WavWriter::create(path, spec)?;
        for &sample in &self.samples {
            writer.write_sample(sample)?;
        }
        writer.finalize()?;
        Ok(())
    }
}

/// Provider trait for TTS backends.
///
/// Each backend (KittenTTS, OpenAI, ElevenLabs, etc.) implements this
/// to expose a unified synthesis interface.
#[async_trait]
pub trait TtsProvider: Send + Sync {
    /// Synthesize speech from text.
    async fn synthesize(&self, request: TtsRequest) -> crate::Result<TtsResponse>;

    /// List available voices.
    fn voices(&self) -> &[Voice];

    /// Provider name (e.g. "kitten", "openai").
    fn name(&self) -> &str;

    /// Check if the provider is ready (model loaded, API key present, etc.).
    async fn is_available(&self) -> bool;
}
