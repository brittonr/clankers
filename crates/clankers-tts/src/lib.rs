//! clankers-tts — Multi-provider text-to-speech router
//!
//! Routes TTS requests to the right backend based on voice name:
//! - **KittenTTS** — local ONNX inference, 8 voices, zero cost, ~25–80 MB models
//! - **OpenAI TTS** — cloud API, 10 voices, requires API key
//!
//! # Quick Start
//!
//! ```no_run
//! use clankers_tts::TtsRouter;
//!
//! # async fn example() -> clankers_tts::Result<()> {
//! let mut router = TtsRouter::new();
//! router.auto_discover();
//!
//! let response = router.synthesize("Hello, world!", "Bella", 1.0).await?;
//! response.write_wav(std::path::Path::new("output.wav"))?;
//! # Ok(())
//! # }
//! ```

pub mod error;
pub mod provider;
pub mod providers;

pub use error::{Error, Result};
pub use provider::{AudioFormat, TtsProvider, TtsRequest, TtsResponse, Voice};

use std::collections::HashMap;
use std::sync::Arc;

use tracing::{info, warn};

/// Multi-provider TTS router.
///
/// Registers backends, resolves voice names to providers, and dispatches
/// synthesis requests.
pub struct TtsRouter {
    providers: Vec<Arc<dyn TtsProvider>>,
    /// voice_id → provider index for fast lookup.
    voice_map: HashMap<String, usize>,
    /// Default provider index (first registered).
    default_provider: Option<usize>,
}

impl TtsRouter {
    /// Create an empty router with no providers.
    pub fn new() -> Self {
        Self {
            providers: Vec::new(),
            voice_map: HashMap::new(),
            default_provider: None,
        }
    }

    /// Register a TTS provider.
    ///
    /// All voices from the provider are added to the voice map.
    /// The first provider registered becomes the default.
    pub fn register(&mut self, provider: Arc<dyn TtsProvider>) {
        let idx = self.providers.len();
        let name = provider.name().to_string();

        for voice in provider.voices() {
            self.voice_map
                .entry(voice.id.to_lowercase())
                .or_insert(idx);
            // Also map the display name
            if voice.name.to_lowercase() != voice.id.to_lowercase() {
                self.voice_map
                    .entry(voice.name.to_lowercase())
                    .or_insert(idx);
            }
        }

        if self.default_provider.is_none() {
            self.default_provider = Some(idx);
        }

        info!(
            "TTS: registered {} with {} voices",
            name,
            provider.voices().len()
        );
        self.providers.push(provider);
    }

    /// Auto-discover and register available TTS providers.
    ///
    /// - KittenTTS: always available (downloads model on first use)
    /// - OpenAI TTS: available when OPENAI_API_KEY is set
    pub fn auto_discover(&mut self) {
        // OpenAI TTS (check first — faster to probe env var)
        if let Some(openai) = providers::openai::OpenAiTtsProvider::from_env() {
            info!("TTS: discovered OpenAI TTS provider");
            self.register(Arc::new(openai));
        }

        // KittenTTS (local, always available)
        #[cfg(feature = "kitten")]
        {
            match providers::kitten::KittenTtsProvider::load_default() {
                Ok(kitten) => {
                    info!("TTS: loaded KittenTTS (local)");
                    self.register(Arc::new(kitten));
                }
                Err(e) => {
                    warn!("TTS: failed to load KittenTTS: {e}");
                }
            }
        }
    }

    /// Synthesize speech from text using the specified voice.
    ///
    /// The voice name is resolved to a provider via the voice map.
    /// Falls back to the default provider if the voice isn't found.
    pub async fn synthesize(
        &self,
        text: &str,
        voice: &str,
        speed: f32,
    ) -> Result<TtsResponse> {
        let provider = self.resolve_provider(voice)?;

        let request = TtsRequest {
            text: text.to_string(),
            voice: voice.to_string(),
            speed,
            format: AudioFormat::default(),
        };

        provider.synthesize(request).await
    }

    /// Synthesize speech and write to a WAV file.
    pub async fn synthesize_to_file(
        &self,
        text: &str,
        output_path: &std::path::Path,
        voice: &str,
        speed: f32,
    ) -> Result<()> {
        let response = self.synthesize(text, voice, speed).await?;
        response.write_wav(output_path)?;
        Ok(())
    }

    /// List all available voices across all providers.
    pub fn list_voices(&self) -> Vec<&Voice> {
        self.providers.iter().flat_map(|p| p.voices()).collect()
    }

    /// List registered provider names.
    pub fn provider_names(&self) -> Vec<&str> {
        self.providers.iter().map(|p| p.name()).collect()
    }

    fn resolve_provider(&self, voice: &str) -> Result<&dyn TtsProvider> {
        // Try exact voice lookup
        if let Some(&idx) = self.voice_map.get(&voice.to_lowercase()) {
            return Ok(self.providers[idx].as_ref());
        }

        // Fall back to default provider
        if let Some(idx) = self.default_provider {
            return Ok(self.providers[idx].as_ref());
        }

        Err(Error::NoProvider {
            voice: voice.to_string(),
        })
    }
}

impl Default for TtsRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for TtsRouter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TtsRouter")
            .field("providers", &self.provider_names())
            .field("voices", &self.voice_map.len())
            .finish()
    }
}
