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
