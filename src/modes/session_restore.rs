//! Restore display blocks from persisted session messages.

use crate::tui::app::App;

/// Rebuild the display blocks from restored session messages so the user
/// can see the prior conversation in the TUI.
pub(crate) fn restore_display_blocks(app: &mut App, messages: &[crate::provider::message::AgentMessage]) {
    use crate::provider::message::AgentMessage;

    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::User(user_msg) => {
                restore_user_message(app, user_msg, i);
            }
            AgentMessage::Assistant(asst_msg) => {
                restore_assistant_message(app, asst_msg);
            }
            AgentMessage::ToolResult(tool_result) => {
                restore_tool_result(app, tool_result);
            }
            _ => {
                // BashExecution, Custom, BranchSummary, CompactionSummary — skip in display
            }
        }
    }
    // Finalize the last active block
    app.finalize_active_block();
}

/// Restore a user message by extracting text content and starting a new block.
fn restore_user_message(app: &mut App, user_msg: &crate::provider::message::UserMessage, index: usize) {
    use crate::provider::message::Content;

    let text = user_msg
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Start a new block for each user message, recording the
    // agent message count at this point for branching support.
    app.start_block_at(text, index, user_msg.timestamp);
}

/// Restore an assistant message by processing its content (text, tool use, thinking).
fn restore_assistant_message(app: &mut App, asst_msg: &crate::provider::message::AssistantMessage) {
    use crate::provider::message::Content;

    for content in &asst_msg.content {
        match content {
            Content::Text { text } => {
                add_text_response(app, text);
            }
            Content::ToolUse { name, input, .. } => {
                add_tool_call_response(app, name, input);
            }
            Content::Thinking { thinking, .. } => {
                add_thinking_response(app, thinking);
            }
            _ => {}
        }
    }
}

/// Add a text response to the active block.
fn add_text_response(app: &mut App, text: &str) {
    use clanker_tui_types::DisplayMessage;
    use clanker_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::Assistant,
            content: text.to_string(),
            tool_name: None,
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Add a tool call response to the active block.
fn add_tool_call_response(app: &mut App, name: &str, input: &serde_json::Value) {
    use clanker_tui_types::DisplayMessage;
    use clanker_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::ToolCall,
            content: name.to_string(),
            tool_name: Some(name.to_string()),
            tool_input: Some(input.clone()),
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Add a thinking response to the active block.
fn add_thinking_response(app: &mut App, thinking: &str) {
    use clanker_tui_types::DisplayMessage;
    use clanker_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::Thinking,
            content: thinking.to_string(),
            tool_name: None,
            tool_input: None,
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Restore a tool result by extracting text and images.
fn restore_tool_result(app: &mut App, tool_result: &crate::provider::message::ToolResultMessage) {
    use clanker_tui_types::DisplayImage;
    use clanker_tui_types::DisplayMessage;
    use clanker_tui_types::MessageRole;

    use crate::provider::message::Content;

    let display = tool_result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Extract images from tool result Content::Image blocks
    let images: Vec<DisplayImage> = tool_result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Image {
                source: crate::provider::message::ImageSource::Base64 { media_type, data },
            } => Some(DisplayImage {
                data: data.clone(),
                media_type: media_type.clone(),
            }),
            _ => None,
        })
        .collect();

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::ToolResult,
            content: display,
            tool_name: None,
            tool_input: None,
            is_error: tool_result.is_error,
            images,
        });
    }
}

#[cfg(test)]
mod tests {
    use chrono::DateTime;
    use chrono::Utc;

    use super::*;

    fn parse_test_timestamp(rfc3339: &str) -> DateTime<Utc> {
        match DateTime::parse_from_rfc3339(rfc3339) {
            Ok(timestamp) => timestamp.with_timezone(&Utc),
            Err(error) => panic!("test timestamp must parse: {error}"),
        }
    }

    fn make_messages() -> Vec<crate::provider::message::AgentMessage> {
        let user_timestamp = parse_test_timestamp("2026-04-22T12:34:56Z");
        let assistant_timestamp = parse_test_timestamp("2026-04-22T12:35:10Z");
        let tool_timestamp = parse_test_timestamp("2026-04-22T12:35:20Z");
        vec![
            crate::provider::message::AgentMessage::User(crate::provider::message::UserMessage {
                id: crate::provider::message::MessageId::new("u1"),
                content: vec![crate::provider::message::Content::Text {
                    text: "hello".to_string(),
                }],
                timestamp: user_timestamp,
            }),
            crate::provider::message::AgentMessage::Assistant(crate::provider::message::AssistantMessage {
                id: crate::provider::message::MessageId::new("a1"),
                content: vec![
                    crate::provider::message::Content::Thinking {
                        thinking: "pondering".to_string(),
                        signature: String::new(),
                    },
                    crate::provider::message::Content::ToolUse {
                        id: "call-1".to_string(),
                        name: "bash".to_string(),
                        input: serde_json::json!({"command": "ls"}),
                    },
                    crate::provider::message::Content::Text {
                        text: "done".to_string(),
                    },
                ],
                model: "test-model".to_string(),
                usage: crate::provider::Usage::default(),
                stop_reason: crate::provider::message::StopReason::Stop,
                timestamp: assistant_timestamp,
            }),
            crate::provider::message::AgentMessage::ToolResult(crate::provider::message::ToolResultMessage {
                id: crate::provider::message::MessageId::new("t1"),
                call_id: "call-1".to_string(),
                tool_name: "bash".to_string(),
                content: vec![crate::provider::message::Content::Text {
                    text: "tool output".to_string(),
                }],
                is_error: false,
                details: None,
                timestamp: tool_timestamp,
            }),
        ]
    }

    #[test]
    fn restore_display_blocks_preserves_started_at_and_finalized_hash() {
        const EXPECTED_RESPONSE_COUNT: usize = 4;
        let mut app = App::new("test".to_string(), "/tmp".to_string(), crate::config::theme::detect_theme());
        let messages = make_messages();

        restore_display_blocks(&mut app, &messages);

        let restored_block = match app.conversation.blocks.last() {
            Some(clanker_tui_types::BlockEntry::Conversation(block)) => block,
            other => panic!("expected restored conversation block, got {other:?}"),
        };

        assert_eq!(restored_block.started_at, parse_test_timestamp("2026-04-22T12:34:56Z"));
        assert!(restored_block.finalized_hash.is_some());
        assert_eq!(restored_block.responses.len(), EXPECTED_RESPONSE_COUNT);
        assert_eq!(restored_block.responses[1].tool_input, Some(serde_json::json!({"command": "ls"})));
    }

    #[test]
    fn restore_display_blocks_does_not_stamp_wall_clock_rebuild_time() {
        let mut first_app = App::new("test".to_string(), "/tmp".to_string(), crate::config::theme::detect_theme());
        let mut second_app = App::new("test".to_string(), "/tmp".to_string(), crate::config::theme::detect_theme());
        let messages = make_messages();

        restore_display_blocks(&mut first_app, &messages);
        restore_display_blocks(&mut second_app, &messages);

        let first_block = match first_app.conversation.blocks.last() {
            Some(clanker_tui_types::BlockEntry::Conversation(block)) => block,
            other => panic!("expected first restored conversation block, got {other:?}"),
        };
        let second_block = match second_app.conversation.blocks.last() {
            Some(clanker_tui_types::BlockEntry::Conversation(block)) => block,
            other => panic!("expected second restored conversation block, got {other:?}"),
        };

        assert_eq!(first_block.started_at, second_block.started_at);
        assert_eq!(first_block.finalized_hash, second_block.finalized_hash);
    }
}
