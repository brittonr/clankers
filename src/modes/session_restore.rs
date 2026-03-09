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
    app.start_block(text, index);
}

/// Restore an assistant message by processing its content (text, tool use, thinking).
fn restore_assistant_message(app: &mut App, asst_msg: &crate::provider::message::AssistantMessage) {
    use crate::provider::message::Content;

    for content in &asst_msg.content {
        match content {
            Content::Text { text } => {
                add_text_response(app, text);
            }
            Content::ToolUse { name, .. } => {
                add_tool_call_response(app, name);
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
    use clankers_tui_types::DisplayMessage;
    use clankers_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::Assistant,
            content: text.to_string(),
            tool_name: None,
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Add a tool call response to the active block.
fn add_tool_call_response(app: &mut App, name: &str) {
    use clankers_tui_types::DisplayMessage;
    use clankers_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::ToolCall,
            content: name.to_string(),
            tool_name: Some(name.to_string()),
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Add a thinking response to the active block.
fn add_thinking_response(app: &mut App, thinking: &str) {
    use clankers_tui_types::DisplayMessage;
    use clankers_tui_types::MessageRole;

    if let Some(ref mut block) = app.conversation.active_block {
        block.responses.push(DisplayMessage {
            role: MessageRole::Thinking,
            content: thinking.to_string(),
            tool_name: None,
            is_error: false,
            images: Vec::new(),
        });
    }
}

/// Restore a tool result by extracting text and images.
fn restore_tool_result(app: &mut App, tool_result: &crate::provider::message::ToolResultMessage) {
    use clankers_tui_types::DisplayImage;
    use clankers_tui_types::DisplayMessage;
    use clankers_tui_types::MessageRole;

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
            is_error: tool_result.is_error,
            images,
        });
    }
}
