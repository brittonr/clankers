//! Image generation tool
//!
//! Generates images via API providers:
//! - Gemini (Google) via the Imagen model
//! - OpenRouter (various providers)
//!
//! Generated images are saved to disk and returned as image content blocks.

#![allow(unexpected_cfgs)]
#![cfg_attr(dylint_lib = "tigerstyle", allow(compound_assertion, ignored_result, no_unwrap, no_panic, no_todo, unjustified_no_todo_allow, no_recursion, unchecked_narrowing, unchecked_division, unbounded_loop, catch_all_on_enum, explicit_defaults, unbounded_channel, unbounded_collection_growth, assertion_density, raw_arithmetic_overflow, sentinel_fallback, acronym_style, bool_naming, negated_predicate, numeric_units, float_for_currency, function_length, nested_conditionals, platform_dependent_cast, usize_in_public_api, too_many_parameters, compound_condition, unjustified_allow, ambiguous_params, ambient_clock, verified_purity, contradictory_time, multi_lock_ordering, reason = "full workspace tigerstyle audit gate: legacy debt documented locally while cleanup proceeds incrementally"))]

use async_trait::async_trait;
use serde_json::Value;
use serde_json::json;

use super::Tool;
use super::ToolContext;
use super::ToolDefinition;
use super::ToolResult;
use super::ToolResultContent;

/// Environment variables for API keys
const GEMINI_API_KEY_ENV: &str = "GEMINI_API_KEY";
const OPENROUTER_API_KEY_ENV: &str = "OPENROUTER_API_KEY";

pub struct ImageGenTool {
    definition: ToolDefinition,
}

impl Default for ImageGenTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ImageGenTool {
    pub fn new() -> Self {
        Self {
            definition: ToolDefinition {
                name: "image_gen".to_string(),
                description: "Generate an image from a text prompt. Requires either \
                    GEMINI_API_KEY or OPENROUTER_API_KEY environment variable. The generated \
                    image is saved to disk and displayed inline."
                    .to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {
                        "prompt": {
                            "type": "string",
                            "description": "Text description of the image to generate"
                        },
                        "provider": {
                            "type": "string",
                            "enum": ["gemini", "openrouter", "auto"],
                            "description": "Which provider to use (default: auto — tries gemini first)",
                            "default": "auto"
                        },
                        "output": {
                            "type": "string",
                            "description": "Output file path (default: generated-<timestamp>.png)"
                        },
                        "size": {
                            "type": "string",
                            "enum": ["256x256", "512x512", "1024x1024"],
                            "description": "Image size (default: 1024x1024)",
                            "default": "1024x1024"
                        }
                    },
                    "required": ["prompt"]
                }),
            },
        }
    }

    async fn generate_gemini(&self, prompt: &str, api_key: &str) -> Result<Vec<u8>, String> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/imagen-3.0-generate-002:predict?key={}",
            api_key
        );

        let body = json!({
            "instances": [{"prompt": prompt}],
            "parameters": {
                "sampleCount": 1
            }
        });

        let resp = client.post(&url).json(&body).send().await.map_err(|e| format!("Gemini request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("Gemini API error ({}): {}", status, text));
        }

        let json: Value = resp.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

        // Extract base64 image data from response
        let b64 = json["predictions"][0]["bytesBase64Encoded"].as_str().ok_or("No image data in Gemini response")?;

        base64_decode(b64)
    }

    async fn generate_openrouter(&self, prompt: &str, api_key: &str) -> Result<Vec<u8>, String> {
        let client = reqwest::Client::new();
        let url = "https://openrouter.ai/api/v1/images/generations";

        let body = json!({
            "model": "openai/dall-e-3",
            "prompt": prompt,
            "n": 1,
            "size": "1024x1024",
            "response_format": "b64_json"
        });

        let resp = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("HTTP-Referer", "https://github.com/clankers")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("OpenRouter request failed: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!("OpenRouter API error ({}): {}", status, text));
        }

        let json: Value = resp.json().await.map_err(|e| format!("Failed to parse response: {}", e))?;

        let b64 = json["data"][0]["b64_json"].as_str().ok_or("No image data in OpenRouter response")?;

        base64_decode(b64)
    }
}

#[async_trait]
impl Tool for ImageGenTool {
    fn definition(&self) -> &ToolDefinition {
        &self.definition
    }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let prompt = match params["prompt"].as_str() {
            Some(p) => p,
            None => return ToolResult::error("Missing required parameter: prompt"),
        };

        let provider = params["provider"].as_str().unwrap_or("auto");
        let output = params["output"].as_str().map(String::from).unwrap_or_else(|| {
            let ts = chrono::Utc::now().format("%Y%m%d-%H%M%S");
            format!("generated-{}.png", ts)
        });

        ctx.emit_progress(&format!("generating image: \"{}\" via {}", prompt, provider));

        let gemini_key = std::env::var(GEMINI_API_KEY_ENV).ok();
        let openrouter_key = std::env::var(OPENROUTER_API_KEY_ENV).ok();

        let image_bytes = match provider {
            "gemini" => {
                let key = match &gemini_key {
                    Some(k) => k,
                    None => return ToolResult::error(format!("Set {} to use Gemini", GEMINI_API_KEY_ENV)),
                };
                self.generate_gemini(prompt, key).await
            }
            "openrouter" => {
                let key = match &openrouter_key {
                    Some(k) => k,
                    None => {
                        return ToolResult::error(format!("Set {} to use OpenRouter", OPENROUTER_API_KEY_ENV));
                    }
                };
                self.generate_openrouter(prompt, key).await
            }
            _ => {
                // Try Gemini first, then OpenRouter
                if let Some(ref key) = gemini_key {
                    match self.generate_gemini(prompt, key).await {
                        Ok(bytes) => Ok(bytes),
                        Err(e) => {
                            tracing::warn!("Gemini failed, trying OpenRouter: {}", e);
                            if let Some(ref key) = openrouter_key {
                                self.generate_openrouter(prompt, key).await
                            } else {
                                Err(format!(
                                    "Gemini failed: {}. Set {} for OpenRouter fallback.",
                                    e, OPENROUTER_API_KEY_ENV
                                ))
                            }
                        }
                    }
                } else if let Some(ref key) = openrouter_key {
                    self.generate_openrouter(prompt, key).await
                } else {
                    return ToolResult::error(format!(
                        "No image generation API key found. Set {} or {}.",
                        GEMINI_API_KEY_ENV, OPENROUTER_API_KEY_ENV
                    ));
                }
            }
        };

        match image_bytes {
            Ok(bytes) => {
                ctx.emit_progress(&format!("saving {} bytes to {}", bytes.len(), output));
                // Save to disk
                if let Err(e) = std::fs::write(&output, &bytes) {
                    return ToolResult::error(format!("Failed to save image to {}: {}", output, e));
                }

                // Also return as base64 image content for inline display
                let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);

                ToolResult {
                    content: vec![
                        ToolResultContent::Text {
                            text: format!(
                                "Image generated and saved to `{}` ({} bytes).\nPrompt: {}",
                                output,
                                bytes.len(),
                                prompt
                            ),
                        },
                        ToolResultContent::Image {
                            media_type: "image/png".to_string(),
                            data: b64,
                        },
                    ],
                    is_error: false,
                    details: None,
                    full_output_path: Some(output),
                }
            }
            Err(e) => ToolResult::error(format!("Image generation failed: {}", e)),
        }
    }
}

fn base64_decode(s: &str) -> Result<Vec<u8>, String> {
    base64::Engine::decode(&base64::engine::general_purpose::STANDARD, s)
        .map_err(|e| format!("Base64 decode error: {}", e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_gen_tool_definition() {
        let tool = ImageGenTool::new();
        assert_eq!(tool.definition().name, "image_gen");
    }

    #[test]
    fn test_base64_roundtrip() {
        let data = b"hello world";
        let encoded = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, data);
        let decoded = base64_decode(&encoded).expect("should decode base64");
        assert_eq!(decoded, data);
    }
}
