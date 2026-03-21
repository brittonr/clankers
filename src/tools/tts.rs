//! Text-to-speech tool
//!
//! Synthesizes speech from text using the clankers-tts multi-provider router.
//! Supports local (KittenTTS) and cloud (OpenAI) backends.
//! Output is saved as a WAV file.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use tokio::sync::OnceCell;

use super::{Tool, ToolContext, ToolDefinition, ToolResult};
#[cfg(test)]
use super::ToolResultContent;

/// Lazily initialized TTS router (shared across all tool invocations).
static TTS_ROUTER: OnceCell<Arc<clankers_tts::TtsRouter>> = OnceCell::const_new();

async fn get_router() -> &'static Arc<clankers_tts::TtsRouter> {
    TTS_ROUTER
        .get_or_init(|| async {
            let mut router = clankers_tts::TtsRouter::new();
            router.auto_discover();
            Arc::new(router)
        })
        .await
}

pub struct TtsTool {
    definition: ToolDefinition,
}

impl Default for TtsTool {
    fn default() -> Self {
        Self::new()
    }
}

impl TtsTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "tts".to_string(),
                description: "Convert text to speech audio. Generates a WAV file from \
                    the given text using a specified voice. Available voices depend on \
                    the configured TTS provider:\n\
                    - KittenTTS (local): Bella, Jasper, Luna, Bruno, Rosie, Hugo, Kiki, Leo\n\
                    - OpenAI (cloud): alloy, ash, ballad, coral, echo, fable, onyx, nova, sage, shimmer"
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "text": {
                            "type": "string",
                            "description": "Text to synthesize into speech"
                        },
                        "voice": {
                            "type": "string",
                            "description": "Voice name (e.g. 'Bella', 'alloy'). Default: 'Bella'",
                            "default": "Bella"
                        },
                        "speed": {
                            "type": "number",
                            "description": "Speech speed multiplier (0.5-2.0, default: 1.0)",
                            "default": 1.0
                        },
                        "output": {
                            "type": "string",
                            "description": "Output WAV file path (default: tts-<timestamp>.wav)"
                        }
                    },
                    "required": ["text"]
                }),
            },
        }
    }
}

#[async_trait]
impl Tool for TtsTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let text = match params["text"].as_str() {
            Some(t) if !t.is_empty() => t,
            _ => return ToolResult::error("Missing required parameter: text"),
        };

        let voice = params["voice"].as_str().unwrap_or("Bella");
        let speed = params["speed"].as_f64().unwrap_or(1.0) as f32;
        let output = params["output"]
            .as_str()
            .map(String::from)
            .unwrap_or_else(|| {
                let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                format!("tts-{ts}.wav")
            });

        ctx.emit_progress(&format!(
            "synthesizing speech: voice={voice}, speed={speed:.1}, {} chars",
            text.len()
        ));

        let router = get_router().await;

        if router.provider_names().is_empty() {
            return ToolResult::error(
                "No TTS providers available. KittenTTS requires espeak-ng installed. \
                 OpenAI TTS requires OPENAI_API_KEY set.",
            );
        }

        // Synthesize on a blocking thread (ONNX inference is CPU-bound)
        let text_owned = text.to_string();
        let voice_owned = voice.to_string();
        let output_clone = output.clone();
        let router_clone = Arc::clone(router);

        let result = tokio::task::spawn_blocking(move || {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let response = router_clone
                    .synthesize(&text_owned, &voice_owned, speed)
                    .await?;
                response.write_wav(std::path::Path::new(&output_clone))?;
                Ok::<_, clankers_tts::Error>((response.duration_ms, response.provider))
            })
        })
        .await;

        match result {
            Ok(Ok((duration_ms, provider))) => {
                let duration_secs = duration_ms as f64 / 1000.0;
                ToolResult::text(format!(
                    "Audio saved to `{output}` ({duration_secs:.1}s, voice={voice}, provider={provider})"
                ))
            }
            Ok(Err(e)) => ToolResult::error(format!("TTS synthesis failed: {e}")),
            Err(e) => ToolResult::error(format!("TTS task panicked: {e}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio_util::sync::CancellationToken;

    use super::*;

    fn make_ctx() -> ToolContext {
        ToolContext::new("test".to_string(), CancellationToken::new(), None)
    }

    fn result_text(result: &ToolResult) -> String {
        result
            .content
            .iter()
            .filter_map(|c| match c {
                ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn definition_name_and_description() {
        let tool = TtsTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "tts");
        assert!(def.description.contains("speech"));
        assert!(def.description.contains("KittenTTS"));
        assert!(def.description.contains("OpenAI"));
    }

    #[test]
    fn definition_schema_has_required_text() {
        let tool = TtsTool::new();
        let schema = &tool.definition().input_schema;
        let required = schema["required"].as_array().unwrap();
        assert_eq!(required.len(), 1);
        assert_eq!(required[0], "text");
    }

    #[test]
    fn definition_schema_properties() {
        let tool = TtsTool::new();
        let props = &tool.definition().input_schema["properties"];
        assert!(props["text"].is_object());
        assert!(props["voice"].is_object());
        assert!(props["speed"].is_object());
        assert!(props["output"].is_object());
    }

    #[test]
    fn default_impl() {
        let tool = TtsTool::default();
        assert_eq!(tool.definition().name, "tts");
    }

    #[tokio::test]
    async fn execute_missing_text_returns_error() {
        let tool = TtsTool::new();
        let ctx = make_ctx();
        let result = tool.execute(&ctx, json!({})).await;
        assert!(result.is_error);
        assert!(result_text(&result).contains("text"));
    }

    #[tokio::test]
    async fn execute_empty_text_returns_error() {
        let tool = TtsTool::new();
        let ctx = make_ctx();
        let result = tool.execute(&ctx, json!({"text": ""})).await;
        assert!(result.is_error);
    }
}
