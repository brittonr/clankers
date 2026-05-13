use chrono::Utc;
use clankers_provider::Usage;
use clankers_provider::message::*;
use clankers_provider::streaming::ContentDelta;
use serde_json::Value;

/// Result of collecting a streamed response
pub(crate) struct CollectedResponse {
    pub(crate) content: Vec<Content>,
    pub(crate) model: String,
    pub(crate) usage: Usage,
    pub(crate) stop_reason: StopReason,
}

const _MESSAGE_MODULE_BOUNDARY: () = ();

pub(crate) fn tool_use_count(content: &[Content]) -> usize {
    content.iter().filter(|block| matches!(block, Content::ToolUse { .. })).count()
}

pub(crate) fn parse_stop_reason(s: &str) -> StopReason {
    match s {
        "end_turn" | "stop" => StopReason::Stop,
        "tool_use" => StopReason::ToolUse,
        "max_tokens" => StopReason::MaxTokens,
        _ => StopReason::Stop,
    }
}

/// Builder for accumulating streaming content blocks
#[derive(Clone)]
pub(crate) struct ContentBlockBuilder {
    content: Content,
    /// For ToolUse blocks, accumulate the raw JSON string
    raw_json: Option<String>,
}

impl ContentBlockBuilder {
    pub(crate) fn new(content: Content) -> Self {
        Self {
            content,
            raw_json: None,
        }
    }

    pub(crate) fn apply_delta(&mut self, delta: &ContentDelta) {
        match (&mut self.content, delta) {
            (Content::Text { text }, ContentDelta::TextDelta { text: delta_text }) => {
                text.push_str(delta_text);
            }
            (
                Content::Thinking { thinking, .. },
                ContentDelta::ThinkingDelta {
                    thinking: delta_thinking,
                },
            ) => {
                thinking.push_str(delta_thinking);
            }
            (Content::Thinking { signature, .. }, ContentDelta::SignatureDelta { signature: sig_delta }) => {
                signature.push_str(sig_delta);
            }
            (Content::ToolUse { .. }, ContentDelta::InputJsonDelta { partial_json }) => {
                self.raw_json.get_or_insert_with(String::new).push_str(partial_json);
            }
            _ => {}
        }
    }

    pub(crate) fn finalize(mut self) -> Content {
        // Parse accumulated JSON for ToolUse
        if let Content::ToolUse {
            ref mut input,
            ref name,
            ..
        } = self.content
        {
            match self.raw_json {
                Some(ref json_str) if !json_str.is_empty() => {
                    match serde_json::from_str::<Value>(json_str) {
                        Ok(parsed) if parsed.is_object() => {
                            *input = parsed;
                        }
                        Ok(parsed) => {
                            // Valid JSON but not an object — wrap it so the tool
                            // still sees something rather than empty {}.
                            tracing::warn!(
                                tool = name,
                                json_type = parsed
                                    .as_str()
                                    .map(|_| "string")
                                    .or(parsed.as_array().map(|_| "array"))
                                    .unwrap_or("other"),
                                "tool input JSON is not an object, wrapping in {{\"_raw\": ...}}",
                            );
                            let mut map = serde_json::Map::new();
                            map.insert("_raw".to_string(), parsed);
                            *input = Value::Object(map);
                        }
                        Err(e) => {
                            tracing::warn!(
                                tool = name,
                                json_len = json_str.len(),
                                error = %e,
                                "failed to parse accumulated tool input JSON",
                            );
                            // Keep the initial empty {} — tool will see missing params
                        }
                    }
                }
                Some(_) => {
                    // raw_json was set but empty (initial empty arguments chunk)
                    tracing::debug!(tool = name, "tool input JSON is empty string");
                }
                None => {
                    // No InputJsonDelta events received at all
                    tracing::debug!(tool = name, "no InputJsonDelta events for tool_use block");
                }
            }
            // Ensure input is always an object
            if !input.is_object() {
                *input = Value::Object(serde_json::Map::new());
            }
        }
        self.content
    }
}

/// Build assistant message from collected response
pub(crate) fn build_assistant_message(collected: &CollectedResponse) -> AssistantMessage {
    AssistantMessage {
        id: MessageId::generate(),
        content: collected.content.clone(),
        model: collected.model.clone(),
        usage: collected.usage.clone(),
        stop_reason: collected.stop_reason.clone(),
        timestamp: Utc::now(),
    }
}

/// Apply output truncation to tool result messages.
///
/// For each tool result, extracts text content, runs it through the truncation
/// layer, and rebuilds the message with truncated text and a temp file path
/// if truncation was applied.
pub(crate) fn apply_output_truncation(
    messages: Vec<ToolResultMessage>,
    config: &clanker_loop::OutputTruncationConfig,
) -> Vec<ToolResultMessage> {
    if !config.enabled {
        return messages;
    }

    messages
        .into_iter()
        .map(|mut msg| {
            // Extract text content blocks, truncate, and rebuild
            let mut truncated_content = Vec::new();
            let mut was_any_truncated = false;

            for block in &msg.content {
                match block {
                    Content::Text { text } => {
                        let result = clanker_loop::truncate_tool_output(text, config);
                        if result.truncated {
                            was_any_truncated = true;
                            tracing::info!(
                                tool = msg.tool_name,
                                original_lines = result.original_lines,
                                original_bytes = result.original_bytes,
                                "Tool output truncated"
                            );
                        }
                        truncated_content.push(Content::Text { text: result.content });
                    }
                    other => truncated_content.push(other.clone()),
                }
            }

            if was_any_truncated {
                msg.content = truncated_content;
            }
            msg
        })
        .collect()
}

/// Convert ToolResultContent to Content
pub(crate) fn tool_result_content_to_message_content(tool_content: &[crate::tool::ToolResultContent]) -> Vec<Content> {
    tool_content
        .iter()
        .map(|tc| match tc {
            crate::tool::ToolResultContent::Text { text } => Content::Text { text: text.clone() },
            crate::tool::ToolResultContent::Image { media_type, data } => Content::Image {
                source: ImageSource::Base64 {
                    media_type: media_type.clone(),
                    data: data.clone(),
                },
            },
        })
        .collect()
}
