//! Neutral controller domain events projected from agent/runtime events.
//!
//! This seam keeps agent/controller/runtime output semantics independent from
//! daemon protocol frames and TUI display DTOs. Transport and display modules
//! project these neutral events at their edges.

use clankers_agent::ToolResultContent;
use clankers_agent::events::AgentEvent;
use clankers_provider::message::Content;
use clankers_provider::streaming::ContentDelta;
use serde_json::Value;

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum ControllerDomainEvent {
    AgentStart,
    AgentEnd,
    ContentBlockStart {
        is_thinking: bool,
    },
    ContentBlockStop,
    TextDelta {
        text: String,
    },
    ThinkingDelta {
        text: String,
    },
    ToolCall {
        tool_name: String,
        call_id: String,
        input: Value,
    },
    ToolStart {
        call_id: String,
        tool_name: String,
    },
    ToolOutput {
        call_id: String,
        text: String,
        images: Vec<DomainImage>,
    },
    ToolProgressUpdate {
        call_id: String,
        message: Option<String>,
    },
    ToolChunk {
        call_id: String,
        content: String,
        content_type: String,
    },
    ToolDone {
        call_id: String,
        text: String,
        images: Vec<DomainImage>,
        is_error: bool,
    },
    UserInput {
        text: String,
        agent_msg_count: usize,
        timestamp_rfc3339: String,
    },
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
    },
    UsageUpdate {
        input_tokens: u64,
        output_tokens: u64,
        cache_read: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DomainImage {
    pub(crate) data: String,
    pub(crate) media_type: String,
}

pub(crate) fn agent_event_to_domain_event(event: &AgentEvent) -> Option<ControllerDomainEvent> {
    match event {
        AgentEvent::AgentStart => Some(ControllerDomainEvent::AgentStart),
        AgentEvent::AgentEnd { .. } => Some(ControllerDomainEvent::AgentEnd),
        AgentEvent::ContentBlockStart { content_block, .. } => Some(ControllerDomainEvent::ContentBlockStart {
            is_thinking: matches!(content_block, Content::Thinking { .. }),
        }),
        AgentEvent::ContentBlockStop { .. } => Some(ControllerDomainEvent::ContentBlockStop),
        AgentEvent::MessageUpdate { delta, .. } => delta_to_domain_event(delta),
        AgentEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(ControllerDomainEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        AgentEvent::ToolExecutionStart { call_id, tool_name } => Some(ControllerDomainEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        AgentEvent::ToolExecutionUpdate { call_id, partial } => {
            let (text, images) = tool_content_to_domain_parts(&partial.content);
            Some(ControllerDomainEvent::ToolOutput {
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
            let (text, images) = tool_content_to_domain_parts(&result.content);
            Some(ControllerDomainEvent::ToolDone {
                call_id: call_id.clone(),
                text,
                images,
                is_error: *is_error,
            })
        }
        AgentEvent::ToolProgressUpdate { call_id, progress } => Some(ControllerDomainEvent::ToolProgressUpdate {
            call_id: call_id.clone(),
            message: progress.message.clone(),
        }),
        AgentEvent::ToolResultChunk { call_id, chunk } => Some(ControllerDomainEvent::ToolChunk {
            call_id: call_id.clone(),
            content: chunk.content.clone(),
            content_type: chunk.content_type.clone(),
        }),
        AgentEvent::UserInput {
            text,
            agent_msg_count,
            timestamp,
        } => Some(ControllerDomainEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
            timestamp_rfc3339: timestamp.to_rfc3339(),
        }),
        AgentEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(ControllerDomainEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        AgentEvent::UsageUpdate { cumulative_usage, .. } => Some(ControllerDomainEvent::UsageUpdate {
            input_tokens: cumulative_usage.input_tokens as u64,
            output_tokens: cumulative_usage.output_tokens as u64,
            cache_read: cumulative_usage.cache_read_input_tokens as u64,
        }),
        _ => None,
    }
}

fn delta_to_domain_event(delta: &ContentDelta) -> Option<ControllerDomainEvent> {
    match delta {
        ContentDelta::TextDelta { text } => Some(ControllerDomainEvent::TextDelta { text: text.clone() }),
        ContentDelta::ThinkingDelta { thinking } => {
            Some(ControllerDomainEvent::ThinkingDelta { text: thinking.clone() })
        }
        _ => None,
    }
}

pub(crate) fn tool_content_to_domain_parts(content: &[ToolResultContent]) -> (String, Vec<DomainImage>) {
    let mut text = String::new();
    let mut images = Vec::new();
    for item in content {
        match item {
            ToolResultContent::Text { text: fragment } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(fragment);
            }
            ToolResultContent::Image { media_type, data } => images.push(DomainImage {
                data: data.clone(),
                media_type: media_type.clone(),
            }),
        }
    }
    (text, images)
}

#[cfg(test)]
mod tests {
    use clankers_agent::ToolResult;
    use clankers_agent::events::AgentEvent;
    use clankers_provider::streaming::ContentDelta;

    use super::*;

    #[test]
    fn projects_agent_streaming_without_protocol_or_tui_types() {
        let event = AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "hello".to_string(),
            },
        };

        assert_eq!(
            agent_event_to_domain_event(&event),
            Some(ControllerDomainEvent::TextDelta {
                text: "hello".to_string(),
            })
        );
    }

    #[test]
    fn projects_tool_receipts_to_neutral_text_and_images() {
        let event = AgentEvent::ToolExecutionEnd {
            call_id: "call-1".to_string(),
            result: ToolResult {
                content: vec![
                    ToolResultContent::Text {
                        text: "line 1".to_string(),
                    },
                    ToolResultContent::Image {
                        media_type: "image/png".to_string(),
                        data: "base64".to_string(),
                    },
                    ToolResultContent::Text {
                        text: "line 2".to_string(),
                    },
                ],
                details: None,
                full_output_path: None,
                is_error: true,
            },
            is_error: true,
        };

        assert_eq!(
            agent_event_to_domain_event(&event),
            Some(ControllerDomainEvent::ToolDone {
                call_id: "call-1".to_string(),
                text: "line 1\nline 2".to_string(),
                images: vec![DomainImage {
                    data: "base64".to_string(),
                    media_type: "image/png".to_string(),
                }],
                is_error: true,
            })
        );
    }

    #[test]
    fn ignores_agent_internal_context_events() {
        let event = AgentEvent::TurnStart { index: 3 };
        assert_eq!(agent_event_to_domain_event(&event), None);
    }
}
