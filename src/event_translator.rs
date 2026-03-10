//! Translate AgentEvent → TuiEvent at the boundary.
//!
//! This module is the single point where agent/provider types are converted
//! into TUI-native events. The TUI crate never imports agent types directly.

use clankers_tui_types::DisplayImage;
use clankers_tui_types::TuiEvent;

use crate::agent::events::AgentEvent;
use crate::provider::message::Content;
use crate::provider::streaming::ContentDelta;
use crate::tools::ToolResultContent;

/// Translate an AgentEvent into zero or more TuiEvents.
///
/// Returns `None` for events the TUI doesn't need (e.g., `Context`,
/// `BeforeAgentStart`, `TurnStart`, `TurnEnd`, `SessionStart`).
pub fn translate(event: &AgentEvent) -> Option<TuiEvent> {
    match event {
        // ── Lifecycle ────────────────────────────────
        AgentEvent::AgentStart => Some(TuiEvent::AgentStart),
        AgentEvent::AgentEnd { .. } => Some(TuiEvent::AgentEnd),

        // ── Streaming ────────────────────────────────
        AgentEvent::ContentBlockStart { content_block, .. } => {
            let is_thinking = matches!(content_block, Content::Thinking { .. });
            Some(TuiEvent::ContentBlockStart { is_thinking })
        }
        AgentEvent::ContentBlockStop { .. } => Some(TuiEvent::ContentBlockStop),
        AgentEvent::MessageUpdate { delta, .. } => match delta {
            ContentDelta::TextDelta { text } => Some(TuiEvent::TextDelta(text.clone())),
            ContentDelta::ThinkingDelta { thinking } => Some(TuiEvent::ThinkingDelta(thinking.clone())),
            _ => None,
        },

        // ── Tool events ──────────────────────────────
        AgentEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(TuiEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        AgentEvent::ToolExecutionStart { call_id, tool_name } => Some(TuiEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        AgentEvent::ToolExecutionUpdate { call_id, partial } => {
            let (text, images) = extract_tool_content(&partial.content);
            Some(TuiEvent::ToolOutput {
                call_id: call_id.clone(),
                text,
                images,
            })
        }
        AgentEvent::ToolExecutionEnd {
            call_id,
            result,
            is_error,
        } => {
            let (text, images) = extract_tool_content(&result.content);
            Some(TuiEvent::ToolDone {
                call_id: call_id.clone(),
                text,
                images,
                is_error: *is_error,
            })
        }
        AgentEvent::ToolProgressUpdate { call_id, progress } => Some(TuiEvent::ToolProgressUpdate {
            call_id: call_id.clone(),
            progress: progress.clone(),
        }),
        AgentEvent::ToolResultChunk { call_id, chunk } => Some(TuiEvent::ToolChunk {
            call_id: call_id.clone(),
            content: chunk.content.clone(),
            content_type: chunk.content_type.clone(),
        }),

        // ── Session events ───────────────────────────
        AgentEvent::UserInput { text, agent_msg_count } => Some(TuiEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
        }),
        AgentEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(TuiEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        AgentEvent::UsageUpdate {
            cumulative_usage,
            turn_usage,
        } => Some(TuiEvent::UsageUpdate {
            total_tokens: cumulative_usage.total_tokens(),
            input_tokens: cumulative_usage.input_tokens,
            output_tokens: cumulative_usage.output_tokens,
            cache_creation_input_tokens: cumulative_usage.cache_creation_input_tokens,
            cache_read_input_tokens: cumulative_usage.cache_read_input_tokens,
            turn_tokens: turn_usage.total_tokens(),
        }),

        // Events the TUI doesn't need
        _ => None,
    }
}

/// Extract text and images from ToolResult content.
fn extract_tool_content(content: &[ToolResultContent]) -> (String, Vec<DisplayImage>) {
    let mut text = String::new();
    let mut images = Vec::new();
    for c in content {
        match c {
            ToolResultContent::Text { text: t } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(t);
            }
            ToolResultContent::Image { media_type, data } => {
                images.push(DisplayImage {
                    data: data.clone(),
                    media_type: media_type.clone(),
                });
            }
        }
    }
    (text, images)
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use serde_json::json;

    use super::*;
    use crate::provider::Usage;
    use crate::provider::message::AssistantMessage;
    use crate::provider::message::Content;
    use crate::provider::message::MessageId;
    use crate::provider::message::StopReason;
    use crate::provider::streaming::ContentDelta;
    use crate::tools::ToolResult;

    #[test]
    fn test_translate_agent_start() {
        let event = AgentEvent::AgentStart;
        let result = translate(&event);
        assert!(matches!(result, Some(TuiEvent::AgentStart)));
    }

    #[test]
    fn test_translate_agent_end() {
        let event = AgentEvent::AgentEnd { messages: vec![] };
        let result = translate(&event);
        assert!(matches!(result, Some(TuiEvent::AgentEnd)));
    }

    #[test]
    fn test_translate_content_block_start_thinking() {
        let event = AgentEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Thinking {
                thinking: "Let me think...".to_string(),
            },
        };
        let result = translate(&event);
        assert!(matches!(result, Some(TuiEvent::ContentBlockStart { is_thinking: true })));
    }

    #[test]
    fn test_translate_content_block_start_text() {
        let event = AgentEvent::ContentBlockStart {
            index: 0,
            content_block: Content::Text {
                text: "Hello".to_string(),
            },
        };
        let result = translate(&event);
        assert!(matches!(result, Some(TuiEvent::ContentBlockStart { is_thinking: false })));
    }

    #[test]
    fn test_translate_message_update_text_delta() {
        let event = AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "Hello".to_string(),
            },
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::TextDelta(text)) => assert_eq!(text, "Hello"),
            _ => panic!("Expected TextDelta, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_message_update_thinking_delta() {
        let event = AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::ThinkingDelta {
                thinking: "Hmm...".to_string(),
            },
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::ThinkingDelta(thinking)) => assert_eq!(thinking, "Hmm..."),
            _ => panic!("Expected ThinkingDelta, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_message_update_input_json_delta_ignored() {
        let event = AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::InputJsonDelta {
                partial_json: r#"{"key""#.to_string(),
            },
        };
        let result = translate(&event);
        assert!(result.is_none());
    }

    #[test]
    fn test_translate_tool_call() {
        let event = AgentEvent::ToolCall {
            tool_name: "bash".to_string(),
            call_id: "call_123".to_string(),
            input: json!({"command": "ls"}),
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::ToolCall {
                tool_name,
                call_id,
                input,
            }) => {
                assert_eq!(tool_name, "bash");
                assert_eq!(call_id, "call_123");
                assert_eq!(input, json!({"command": "ls"}));
            }
            _ => panic!("Expected ToolCall, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_tool_execution_start() {
        let event = AgentEvent::ToolExecutionStart {
            call_id: "call_456".to_string(),
            tool_name: "read".to_string(),
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::ToolStart { call_id, tool_name }) => {
                assert_eq!(call_id, "call_456");
                assert_eq!(tool_name, "read");
            }
            _ => panic!("Expected ToolStart, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_tool_execution_end_success() {
        let event = AgentEvent::ToolExecutionEnd {
            call_id: "call_789".to_string(),
            result: ToolResult::text("Output text"),
            is_error: false,
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::ToolDone {
                call_id,
                text,
                images,
                is_error,
            }) => {
                assert_eq!(call_id, "call_789");
                assert_eq!(text, "Output text");
                assert!(images.is_empty());
                assert!(!is_error);
            }
            _ => panic!("Expected ToolDone, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_tool_execution_end_error() {
        let event = AgentEvent::ToolExecutionEnd {
            call_id: "call_err".to_string(),
            result: ToolResult::error("Something went wrong"),
            is_error: true,
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::ToolDone {
                call_id,
                text,
                images,
                is_error,
            }) => {
                assert_eq!(call_id, "call_err");
                assert_eq!(text, "Something went wrong");
                assert!(images.is_empty());
                assert!(is_error);
            }
            _ => panic!("Expected ToolDone with error, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_user_input() {
        let event = AgentEvent::UserInput {
            text: "Hello, agent!".to_string(),
            agent_msg_count: 5,
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::UserInput { text, agent_msg_count }) => {
                assert_eq!(text, "Hello, agent!");
                assert_eq!(agent_msg_count, 5);
            }
            _ => panic!("Expected UserInput, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_usage_update() {
        let event = AgentEvent::UsageUpdate {
            cumulative_usage: Usage {
                input_tokens: 1000,
                output_tokens: 500,
                cache_creation_input_tokens: 200,
                cache_read_input_tokens: 300,
            },
            turn_usage: Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_creation_input_tokens: 20,
                cache_read_input_tokens: 30,
            },
        };
        let result = translate(&event);
        match result {
            Some(TuiEvent::UsageUpdate {
                total_tokens,
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
                turn_tokens,
            }) => {
                assert_eq!(total_tokens, 1500); // 1000 + 500
                assert_eq!(input_tokens, 1000);
                assert_eq!(output_tokens, 500);
                assert_eq!(cache_creation_input_tokens, 200);
                assert_eq!(cache_read_input_tokens, 300);
                assert_eq!(turn_tokens, 150); // 100 + 50
            }
            _ => panic!("Expected UsageUpdate, got {:?}", result),
        }
    }

    #[test]
    fn test_translate_ignored_events() {
        let ignored_events = vec![
            AgentEvent::SessionStart {
                session_id: "session_123".to_string(),
            },
            AgentEvent::TurnStart { index: 1 },
            AgentEvent::TurnEnd {
                index: 1,
                message: AssistantMessage {
                    id: MessageId::new("msg_1"),
                    content: vec![],
                    model: "claude-3-5-sonnet-20241022".to_string(),
                    usage: Usage::default(),
                    stop_reason: StopReason::Stop,
                    timestamp: Utc::now(),
                },
                tool_results: vec![],
            },
            AgentEvent::BeforeAgentStart {
                prompt: "Test".to_string(),
                system_prompt: "System".to_string(),
            },
            AgentEvent::Context { messages: vec![] },
            AgentEvent::ContentBlockStop { index: 0 },
        ];

        for event in ignored_events {
            let result = translate(&event);
            // ContentBlockStop is actually not ignored - it maps to TuiEvent::ContentBlockStop
            if matches!(event, AgentEvent::ContentBlockStop { .. }) {
                assert!(
                    matches!(result, Some(TuiEvent::ContentBlockStop)),
                    "ContentBlockStop should translate to TuiEvent::ContentBlockStop"
                );
            } else {
                assert!(result.is_none(), "Event {:?} should be ignored, got {:?}", event, result);
            }
        }
    }

    #[test]
    fn test_extract_tool_content_mixed() {
        let content = vec![
            ToolResultContent::Text {
                text: "First line".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/png".to_string(),
                data: "base64data1".to_string(),
            },
            ToolResultContent::Text {
                text: "Second line".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/jpeg".to_string(),
                data: "base64data2".to_string(),
            },
        ];

        let (text, images) = extract_tool_content(&content);

        assert_eq!(text, "First line\nSecond line");
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].media_type, "image/png");
        assert_eq!(images[0].data, "base64data1");
        assert_eq!(images[1].media_type, "image/jpeg");
        assert_eq!(images[1].data, "base64data2");
    }

    #[test]
    fn test_extract_tool_content_empty() {
        let content = vec![];
        let (text, images) = extract_tool_content(&content);

        assert_eq!(text, "");
        assert!(images.is_empty());
    }

    #[test]
    fn test_extract_tool_content_text_only() {
        let content = vec![
            ToolResultContent::Text {
                text: "Line 1".to_string(),
            },
            ToolResultContent::Text {
                text: "Line 2".to_string(),
            },
            ToolResultContent::Text {
                text: "Line 3".to_string(),
            },
        ];

        let (text, images) = extract_tool_content(&content);

        assert_eq!(text, "Line 1\nLine 2\nLine 3");
        assert!(images.is_empty());
    }

    #[test]
    fn test_extract_tool_content_images_only() {
        let content = vec![
            ToolResultContent::Image {
                media_type: "image/png".to_string(),
                data: "data1".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/gif".to_string(),
                data: "data2".to_string(),
            },
        ];

        let (text, images) = extract_tool_content(&content);

        assert_eq!(text, "");
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].media_type, "image/png");
        assert_eq!(images[1].media_type, "image/gif");
    }
}
