//! Restore display blocks from persisted session messages.

use crate::tui::app::App;

/// Rebuild the display blocks from restored session messages so the user
/// can see the prior conversation in the TUI.
pub(crate) fn restore_display_blocks(app: &mut App, messages: &[crate::provider::message::AgentMessage]) {
    use crate::provider::message::AgentMessage;
    use crate::provider::message::Content;
    use crate::tui::app::DisplayMessage;
    use crate::tui::app::MessageRole;

    for (i, msg) in messages.iter().enumerate() {
        match msg {
            AgentMessage::User(user_msg) => {
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
                app.start_block(text, i);
            }
            AgentMessage::Assistant(asst_msg) => {
                // Add responses to the current active block
                for content in &asst_msg.content {
                    match content {
                        Content::Text { text } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::Assistant,
                                    content: text.clone(),
                                    tool_name: None,
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        Content::ToolUse { name, .. } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::ToolCall,
                                    content: name.clone(),
                                    tool_name: Some(name.clone()),
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        Content::Thinking { thinking, .. } => {
                            if let Some(ref mut block) = app.active_block {
                                block.responses.push(DisplayMessage {
                                    role: MessageRole::Thinking,
                                    content: thinking.clone(),
                                    tool_name: None,
                                    is_error: false,
                                    images: Vec::new(),
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
            AgentMessage::ToolResult(tool_result) => {
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
                let images: Vec<crate::tui::app::DisplayImage> = tool_result
                    .content
                    .iter()
                    .filter_map(|c| match c {
                        Content::Image {
                            source: crate::provider::message::ImageSource::Base64 { media_type, data },
                        } => Some(crate::tui::app::DisplayImage {
                            data: data.clone(),
                            media_type: media_type.clone(),
                        }),
                        _ => None,
                    })
                    .collect();
                if let Some(ref mut block) = app.active_block {
                    block.responses.push(DisplayMessage {
                        role: MessageRole::ToolResult,
                        content: display,
                        tool_name: None,
                        is_error: tool_result.is_error,
                        images,
                    });
                }
            }
            _ => {
                // BashExecution, Custom, BranchSummary, CompactionSummary — skip in display
            }
        }
    }
    // Finalize the last active block
    app.finalize_active_block();
}

