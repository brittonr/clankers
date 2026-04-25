//! KittenTTS provider — local ONNX-based TTS.
//!
//! Wraps the `kittentts` crate for zero-latency, zero-cost local synthesis.
//! Models are downloaded from HuggingFace on first use (~25–80 MB).

use std::sync::Mutex;

use async_trait::async_trait;
use tracing::debug;
use tracing::info;

use crate::error::Error;
use crate::error::Result;
use crate::provider::TtsProvider;
use crate::provider::TtsRequest;
use crate::provider::TtsResponse;
use crate::provider::Voice;

/// Phonemize text to IPA via espeak-ng subprocess.
///
/// The `kittentts` crate's built-in espeak feature requires a static
/// libespeak-ng.a, which NixOS doesn't ship. We call espeak-ng as a
/// subprocess instead — same output, works everywhere espeak-ng is installed.
fn phonemize(text: &str) -> Result<String> {
    let output = std::process::Command::new("espeak-ng")
        .args(["--ipa", "-v", "en-us", "-q", text])
        .output()
        .map_err(|e| Error::Provider {
            message: format!("espeak-ng not found: {e}. Install espeak-ng for TTS."),
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Provider {
            message: format!("espeak-ng failed: {stderr}"),
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Available KittenTTS model variants.
#[derive(Debug, Clone, Copy)]
pub enum KittenModel {
    /// 15M parameters, 25 MB (int8) / 56 MB (fp32). Fastest.
    Nano,
    /// 40M parameters, 41 MB. Balanced speed/quality.
    Micro,
    /// 80M parameters, 80 MB. Highest quality.
    Mini,
}

impl KittenModel {
    fn repo_id(self) -> &'static str {
        match self {
            Self::Nano => "KittenML/kitten-tts-nano-0.8",
            Self::Micro => "KittenML/kitten-tts-micro-0.8",
            Self::Mini => "KittenML/kitten-tts-mini-0.8",
        }
    }
}

/// KittenTTS provider wrapping the `kittentts` crate.
pub struct KittenTtsProvider {
    model: Mutex<kittentts::KittenTTS>,
    voices: Vec<Voice>,
    model_variant: KittenModel,
}

impl KittenTtsProvider {
    /// Load a KittenTTS model from HuggingFace Hub.
    ///
    /// Downloads and caches the model on first call (~25–80 MB depending
    /// on variant). Subsequent calls load from cache.
    pub fn load(variant: KittenModel) -> Result<Self> {
        let repo_id = variant.repo_id();
        info!("loading KittenTTS model: {repo_id}");

        let model = kittentts::download::load_from_hub(repo_id).map_err(|e| Error::Provider {
            message: format!("failed to load KittenTTS model {repo_id}: {e}"),
        })?;

        let voices = vec![
            voice("Bella", "kitten"),
            voice("Jasper", "kitten"),
            voice("Luna", "kitten"),
            voice("Bruno", "kitten"),
            voice("Rosie", "kitten"),
            voice("Hugo", "kitten"),
            voice("Kiki", "kitten"),
            voice("Leo", "kitten"),
        ];

        Ok(Self {
            model: Mutex::new(model),
            voices,
            model_variant: variant,
        })
    }

    /// Load the default model (Nano — smallest, fastest).
    pub fn load_default() -> Result<Self> {
        Self::load(KittenModel::Nano)
    }
}

fn voice(name: &str, provider: &str) -> Voice {
    Voice {
        id: name.to_string(),
        name: name.to_string(),
        provider: provider.to_string(),
    }
}

#[async_trait]
impl TtsProvider for KittenTtsProvider {
    async fn synthesize(&self, request: TtsRequest) -> Result<TtsResponse> {
        let voice = request.voice.clone();
        let speed = request.speed;
        let text = request.text.clone();

        // Phonemize via espeak-ng subprocess
        let ipa = phonemize(&text)?;
        debug!("phonemized: {text:?} → {ipa:?}");

        // kittentts::KittenTTS::generate_from_ipa is blocking (ONNX inference),
        // so we hold the lock for the duration.
        let model = self.model.lock().map_err(|e| Error::Provider {
            message: format!("model lock poisoned: {e}"),
        })?;

        let text_len = text.len();
        let samples = model.generate_from_ipa(&ipa, &voice, speed, text_len).map_err(|e| Error::Provider {
            message: format!("KittenTTS synthesis failed: {e}"),
        })?;

        let sample_rate = kittentts::SAMPLE_RATE;
        let sample_count = samples.len() as u64;
        let duration_ms = sample_count.saturating_mul(1000).checked_div(u64::from(sample_rate)).unwrap_or(0);

        Ok(TtsResponse {
            samples,
            sample_rate,
            duration_ms,
            voice: request.voice,
            provider: "kitten".to_string(),
        })
    }

    fn voices(&self) -> &[Voice] {
        &self.voices
    }

    fn name(&self) -> &str {
        "kitten"
    }

    async fn is_available(&self) -> bool {
        true // always available once loaded
    }
}

impl std::fmt::Debug for KittenTtsProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("KittenTtsProvider")
            .field("model", &self.model_variant)
            .field("voices", &self.voices.len())
            .finish()
    }
}
