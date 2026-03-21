//! OpenAI TTS provider — cloud-based synthesis via /v1/audio/speech.
//!
//! Supports models: tts-1, tts-1-hd, gpt-4o-mini-tts
//! Voices: alloy, ash, ballad, coral, echo, fable, onyx, nova, sage, shimmer

use async_trait::async_trait;
use tracing::debug;

use crate::error::{Error, Result};
use crate::provider::{TtsProvider, TtsRequest, TtsResponse, Voice};

const OPENAI_TTS_URL: &str = "https://api.openai.com/v1/audio/speech";
const SAMPLE_RATE: u32 = 24_000;

/// OpenAI TTS model variants.
#[derive(Debug, Clone, Default)]
pub enum OpenAiModel {
    /// Standard quality, low latency.
    #[default]
    Tts1,
    /// High definition, higher latency.
    Tts1Hd,
    /// GPT-4o Mini TTS (newest).
    Gpt4oMiniTts,
}

impl OpenAiModel {
    fn as_str(&self) -> &str {
        match self {
            Self::Tts1 => "tts-1",
            Self::Tts1Hd => "tts-1-hd",
            Self::Gpt4oMiniTts => "gpt-4o-mini-tts",
        }
    }
}

/// OpenAI TTS provider.
pub struct OpenAiTtsProvider {
    api_key: String,
    model: OpenAiModel,
    client: reqwest::Client,
    voices: Vec<Voice>,
}

impl OpenAiTtsProvider {
    /// Create a new OpenAI TTS provider.
    pub fn new(api_key: String, model: OpenAiModel) -> Self {
        let voices = [
            "alloy", "ash", "ballad", "coral", "echo", "fable", "onyx", "nova", "sage", "shimmer",
        ]
        .iter()
        .map(|&name| Voice {
            id: name.to_string(),
            name: name.to_string(),
            provider: "openai-tts".to_string(),
        })
        .collect();

        Self {
            api_key,
            model,
            client: reqwest::Client::new(),
            voices,
        }
    }

    /// Create from the OPENAI_API_KEY environment variable.
    pub fn from_env() -> Option<Self> {
        let key = std::env::var("OPENAI_API_KEY").ok()?;
        if key.is_empty() {
            return None;
        }
        Some(Self::new(key, OpenAiModel::default()))
    }
}

#[async_trait]
impl TtsProvider for OpenAiTtsProvider {
    async fn synthesize(&self, request: TtsRequest) -> Result<TtsResponse> {
        debug!("OpenAI TTS: synthesizing with voice={}", request.voice);

        let body = serde_json::json!({
            "model": self.model.as_str(),
            "input": request.text,
            "voice": request.voice,
            "speed": request.speed,
            "response_format": "pcm",
        });

        let resp = self
            .client
            .post(OPENAI_TTS_URL)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            let status = resp.status().as_u16();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Provider {
                message: format!("OpenAI TTS HTTP {status}: {body}"),
            });
        }

        // OpenAI returns raw PCM16 at 24kHz mono when format=pcm
        let bytes = resp.bytes().await?;
        let samples: Vec<f32> = bytes
            .chunks_exact(2)
            .map(|chunk| {
                let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
                f32::from(sample) / 32768.0
            })
            .collect();

        let duration_ms = (samples.len() as u64 * 1000) / u64::from(SAMPLE_RATE);

        Ok(TtsResponse {
            samples,
            sample_rate: SAMPLE_RATE,
            duration_ms,
            voice: request.voice,
            provider: "openai-tts".to_string(),
        })
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn name(&self) -> &str {
        "openai-tts"
    }

    async fn is_available(&self) -> bool {
        !self.api_key.is_empty()
    }
}

impl std::fmt::Debug for OpenAiTtsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OpenAiTtsProvider")
            .field("model", &self.model)
            .field("has_key", &!self.api_key.is_empty())
            .finish()
    }
}
