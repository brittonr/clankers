//! Convert AgentEvent → DaemonEvent at the controller boundary.
//!
//! This is the daemon-side equivalent of event_translator.rs in the main crate,
//! but produces protocol DaemonEvents instead of TuiEvents.

use clankers_agent::ToolResultContent;
use clankers_agent::events::AgentEvent;
use clankers_protocol::event::DaemonEvent;
use clankers_protocol::types::ImageData;
use clankers_provider::streaming::ContentDelta;

/// Translate an AgentEvent into a DaemonEvent (or None for events clients
/// don't need, like Context, BeforeAgentStart, TurnStart, etc.).
pub fn agent_event_to_daemon_event(event: &AgentEvent) -> Option<DaemonEvent> {
    match event {
        // ── Lifecycle ────────────────────────────────
        AgentEvent::AgentStart => Some(DaemonEvent::AgentStart),
        AgentEvent::AgentEnd { .. } => Some(DaemonEvent::AgentEnd),

        // ── Streaming ────────────────────────────────
        AgentEvent::ContentBlockStart { content_block, .. } => {
            let is_thinking = matches!(content_block, clankers_provider::message::Content::Thinking { .. });
            Some(DaemonEvent::ContentBlockStart { is_thinking })
        }
        AgentEvent::ContentBlockStop { .. } => Some(DaemonEvent::ContentBlockStop),
        AgentEvent::MessageUpdate { delta, .. } => match delta {
            ContentDelta::TextDelta { text } => Some(DaemonEvent::TextDelta { text: text.clone() }),
            ContentDelta::ThinkingDelta { thinking } => Some(DaemonEvent::ThinkingDelta { text: thinking.clone() }),
            _ => None,
        },

        // ── Tool events ──────────────────────────────
        AgentEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(DaemonEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        AgentEvent::ToolExecutionStart { call_id, tool_name } => Some(DaemonEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        AgentEvent::ToolExecutionUpdate { call_id, partial } => {
            let (text, images) = extract_tool_content(&partial.content);
            Some(DaemonEvent::ToolOutput {
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
            Some(DaemonEvent::ToolDone {
                call_id: call_id.clone(),
                text,
                images,
                is_error: *is_error,
            })
        }
        AgentEvent::ToolProgressUpdate { call_id, progress } => {
            // ToolProgress contains Instant which isn't serializable.
            // Serialize the message field only.
            let progress_json = serde_json::json!({
                "message": progress.message,
            });
            Some(DaemonEvent::ToolProgressUpdate {
                call_id: call_id.clone(),
                progress: progress_json,
            })
        }
        AgentEvent::ToolResultChunk { call_id, chunk } => Some(DaemonEvent::ToolChunk {
            call_id: call_id.clone(),
            content: chunk.content.clone(),
            content_type: chunk.content_type.clone(),
        }),

        // ── Session events ───────────────────────────
        AgentEvent::UserInput { text, agent_msg_count } => Some(DaemonEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
        }),
        AgentEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(DaemonEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        AgentEvent::UsageUpdate { cumulative_usage, .. } => Some(DaemonEvent::UsageUpdate {
            input_tokens: cumulative_usage.input_tokens as u64,
            output_tokens: cumulative_usage.output_tokens as u64,
            cache_read: cumulative_usage.cache_read_input_tokens as u64,
            model: String::new(), // filled in by controller if needed
        }),

        // Events the daemon doesn't forward to clients
        _ => None,
    }
}

/// Convert DaemonEvent into a TuiEvent for the client side.
///
/// This is the inverse of `agent_event_to_daemon_event` — the TUI client
/// calls this to produce TuiEvents from the socket stream.
pub fn daemon_event_to_tui_event(event: &DaemonEvent) -> Option<clankers_tui_types::TuiEvent> {
    match event {
        DaemonEvent::AgentStart => Some(clankers_tui_types::TuiEvent::AgentStart),
        DaemonEvent::AgentEnd => Some(clankers_tui_types::TuiEvent::AgentEnd),

        DaemonEvent::ContentBlockStart { is_thinking } => Some(clankers_tui_types::TuiEvent::ContentBlockStart {
            is_thinking: *is_thinking,
        }),
        DaemonEvent::ContentBlockStop => Some(clankers_tui_types::TuiEvent::ContentBlockStop),

        DaemonEvent::TextDelta { text } => Some(clankers_tui_types::TuiEvent::TextDelta(text.clone())),
        DaemonEvent::ThinkingDelta { text } => Some(clankers_tui_types::TuiEvent::ThinkingDelta(text.clone())),

        DaemonEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(clankers_tui_types::TuiEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        DaemonEvent::ToolStart { call_id, tool_name } => Some(clankers_tui_types::TuiEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        DaemonEvent::ToolOutput { call_id, text, images } => Some(clankers_tui_types::TuiEvent::ToolOutput {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images
                .iter()
                .map(|i| clankers_tui_types::DisplayImage {
                    data: i.data.clone(),
                    media_type: i.media_type.clone(),
                })
                .collect(),
        }),
        DaemonEvent::ToolProgressUpdate { call_id, progress } => {
            // Best-effort conversion — structured progress may not round-trip perfectly
            // ToolProgress contains non-serializable Instant, so we reconstruct
            let _ = progress;
            Some(clankers_tui_types::TuiEvent::ToolProgressUpdate {
                call_id: call_id.clone(),
                progress: clankers_tui_types::ToolProgress {
                    kind: clankers_tui_types::ProgressKind::Phase {
                        name: "progress".to_string(),
                        step: 0,
                        total_steps: None,
                    },
                    message: None,
                    timestamp: std::time::Instant::now(),
                },
            })
        }
        DaemonEvent::ToolChunk {
            call_id,
            content,
            content_type,
        } => Some(clankers_tui_types::TuiEvent::ToolChunk {
            call_id: call_id.clone(),
            content: content.clone(),
            content_type: content_type.clone(),
        }),
        DaemonEvent::ToolDone {
            call_id,
            text,
            images,
            is_error,
        } => Some(clankers_tui_types::TuiEvent::ToolDone {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images
                .iter()
                .map(|i| clankers_tui_types::DisplayImage {
                    data: i.data.clone(),
                    media_type: i.media_type.clone(),
                })
                .collect(),
            is_error: *is_error,
        }),

        DaemonEvent::UserInput { text, agent_msg_count } => Some(clankers_tui_types::TuiEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
        }),
        DaemonEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(clankers_tui_types::TuiEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        DaemonEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            cache_read,
            ..
        } => Some(clankers_tui_types::TuiEvent::UsageUpdate {
            total_tokens: (*input_tokens + *output_tokens) as usize,
            input_tokens: *input_tokens as usize,
            output_tokens: *output_tokens as usize,
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: *cache_read as usize,
            turn_tokens: 0,
        }),

        // Events that don't map to TuiEvent — handled by the client directly
        _ => None,
    }
}

/// Extract text and images from ToolResult content.
fn extract_tool_content(content: &[ToolResultContent]) -> (String, Vec<ImageData>) {
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
                images.push(ImageData {
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
    use clankers_agent::ToolResult;

    use super::*;

    #[test]
    fn test_agent_start_converts() {
        let event = AgentEvent::AgentStart;
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(result, Some(DaemonEvent::AgentStart)));
    }

    #[test]
    fn test_agent_end_converts() {
        let event = AgentEvent::AgentEnd { messages: vec![] };
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(result, Some(DaemonEvent::AgentEnd)));
    }

    #[test]
    fn test_text_delta_converts() {
        let event = AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: "hello".to_string(),
            },
        };
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(result, Some(DaemonEvent::TextDelta { text }) if text == "hello"));
    }

    #[test]
    fn test_tool_call_converts() {
        let event = AgentEvent::ToolCall {
            tool_name: "bash".to_string(),
            call_id: "c1".to_string(),
            input: serde_json::json!({"command": "ls"}),
        };
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(result, Some(DaemonEvent::ToolCall { tool_name, .. }) if tool_name == "bash"));
    }

    #[test]
    fn test_tool_done_converts() {
        let event = AgentEvent::ToolExecutionEnd {
            call_id: "c1".to_string(),
            result: ToolResult::text("output"),
            is_error: false,
        };
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(result, Some(DaemonEvent::ToolDone { text, is_error: false, .. }) if text == "output"));
    }

    #[test]
    fn test_ignored_events() {
        let ignored = vec![
            AgentEvent::SessionStart {
                session_id: "s1".to_string(),
            },
            AgentEvent::TurnStart { index: 1 },
        ];
        for event in ignored {
            assert!(agent_event_to_daemon_event(&event).is_none());
        }
    }

    #[test]
    fn test_daemon_to_tui_agent_start() {
        let event = DaemonEvent::AgentStart;
        let result = daemon_event_to_tui_event(&event);
        assert!(matches!(result, Some(clankers_tui_types::TuiEvent::AgentStart)));
    }

    #[test]
    fn test_daemon_to_tui_text_delta() {
        let event = DaemonEvent::TextDelta {
            text: "hello".to_string(),
        };
        let result = daemon_event_to_tui_event(&event);
        assert!(matches!(result, Some(clankers_tui_types::TuiEvent::TextDelta(t)) if t == "hello"));
    }

    #[test]
    fn test_daemon_to_tui_non_tui_events() {
        // Events that don't map to TuiEvent
        let non_tui = vec![
            DaemonEvent::ConfirmRequest {
                request_id: "r1".to_string(),
                command: "ls".to_string(),
                working_dir: "/".to_string(),
            },
            DaemonEvent::SessionInfo {
                session_id: "s1".to_string(),
                model: "m".to_string(),
                system_prompt_hash: "h".to_string(),
            },
            DaemonEvent::SystemMessage {
                text: "hi".to_string(),
                is_error: false,
            },
            DaemonEvent::HistoryEnd,
        ];
        for event in non_tui {
            assert!(daemon_event_to_tui_event(&event).is_none());
        }
    }

    #[test]
    fn test_extract_tool_content_mixed() {
        let content = vec![
            ToolResultContent::Text {
                text: "line1".to_string(),
            },
            ToolResultContent::Image {
                media_type: "image/png".to_string(),
                data: "base64".to_string(),
            },
            ToolResultContent::Text {
                text: "line2".to_string(),
            },
        ];

        let (text, images) = extract_tool_content(&content);
        assert_eq!(text, "line1\nline2");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].media_type, "image/png");
    }
}
