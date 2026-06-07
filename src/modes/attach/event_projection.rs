//! Attach-side projection from daemon/session protocol events into TUI events.
//!
//! `clankers-controller` owns semantic/protocol projection only. Display DTO
//! construction stays in the root attach/TUI edge so controller remains free of
//! `clanker-tui-types`.

use chrono::DateTime;
use chrono::Utc;
use clanker_message::SemanticEvent;
use clanker_tui_types::DisplayImage;
use clanker_tui_types::ProgressKind;
use clanker_tui_types::ToolProgress;
use clanker_tui_types::TuiEvent;
use clankers_protocol::DaemonEvent;
use clankers_protocol::types::ImageData;

#[allow(dead_code)]
pub(super) fn semantic_event_to_tui_event(event: &SemanticEvent) -> Option<TuiEvent> {
    clankers_controller::convert::semantic_event_to_daemon_event(event)
        .and_then(|event| daemon_event_to_tui_event(&event))
}

/// Convert `DaemonEvent` into `TuiEvent` for attached clients.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        function_length,
        reason = "sequential protocol-to-display projection over many event variants"
    )
)]
pub(super) fn daemon_event_to_tui_event(event: &DaemonEvent) -> Option<TuiEvent> {
    match event {
        DaemonEvent::AgentStart => Some(TuiEvent::AgentStart),
        DaemonEvent::AgentEnd => Some(TuiEvent::AgentEnd),
        DaemonEvent::ContentBlockStart { is_thinking } => Some(TuiEvent::ContentBlockStart {
            is_thinking: *is_thinking,
        }),
        DaemonEvent::ContentBlockStop => Some(TuiEvent::ContentBlockStop),
        DaemonEvent::TextDelta { text } => Some(TuiEvent::TextDelta(text.clone())),
        DaemonEvent::ThinkingDelta { text } => Some(TuiEvent::ThinkingDelta(text.clone())),
        DaemonEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(TuiEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        DaemonEvent::ToolStart { call_id, tool_name } => Some(TuiEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        DaemonEvent::ToolOutput { call_id, text, images } => Some(TuiEvent::ToolOutput {
            call_id: call_id.clone(),
            text: text.clone(),
            images: protocol_images_to_display(images),
        }),
        DaemonEvent::ToolProgressUpdate { call_id, progress } => {
            let _ = progress;
            Some(TuiEvent::ToolProgressUpdate {
                call_id: call_id.clone(),
                progress: ToolProgress {
                    kind: ProgressKind::Phase {
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
        } => Some(TuiEvent::ToolChunk {
            call_id: call_id.clone(),
            content: content.clone(),
            content_type: content_type.clone(),
        }),
        DaemonEvent::ToolDone {
            call_id,
            text,
            images,
            is_error,
        } => Some(TuiEvent::ToolDone {
            call_id: call_id.clone(),
            text: text.clone(),
            images: protocol_images_to_display(images),
            is_error: *is_error,
        }),
        DaemonEvent::UserInput {
            text,
            agent_msg_count,
            timestamp,
        } => Some(TuiEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
            timestamp: parse_user_input_timestamp(timestamp),
        }),
        DaemonEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(TuiEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        DaemonEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            cache_read,
            ..
        } => Some(TuiEvent::UsageUpdate {
            total_tokens: usize::try_from(*input_tokens + *output_tokens).unwrap_or(usize::MAX),
            input_tokens: usize::try_from(*input_tokens).unwrap_or(usize::MAX),
            output_tokens: usize::try_from(*output_tokens).unwrap_or(usize::MAX),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: usize::try_from(*cache_read).unwrap_or(usize::MAX),
            turn_tokens: 0,
        }),
        _ => None,
    }
}

/// Convert a stored transcript `AgentMessage` into TUI events for history replay.
///
/// Replay keeps the active block open across assistant and tool-result messages
/// until the next user prompt or the explicit history-end marker finalizes it.
pub(super) fn agent_message_to_tui_events(msg: &clanker_message::transcript::AgentMessage) -> Vec<TuiEvent> {
    use clanker_message::Content;
    use clanker_message::transcript::AgentMessage;

    match msg {
        AgentMessage::User(m) => vec![TuiEvent::UserInput {
            text: extract_user_text(&m.content),
            agent_msg_count: 0,
            timestamp: m.timestamp,
        }],
        AgentMessage::Assistant(m) => {
            let mut events = vec![TuiEvent::AgentStart];
            for block in &m.content {
                match block {
                    Content::Text { text } => {
                        events.push(TuiEvent::ContentBlockStart { is_thinking: false });
                        events.push(TuiEvent::TextDelta(text.clone()));
                        events.push(TuiEvent::ContentBlockStop);
                    }
                    Content::Thinking { thinking, .. } => {
                        events.push(TuiEvent::ContentBlockStart { is_thinking: true });
                        events.push(TuiEvent::ThinkingDelta(thinking.clone()));
                        events.push(TuiEvent::ContentBlockStop);
                    }
                    Content::ToolUse { id, name, input } => {
                        events.push(TuiEvent::ToolCall {
                            tool_name: name.clone(),
                            call_id: id.clone(),
                            input: input.clone(),
                        });
                        events.push(TuiEvent::ToolStart {
                            call_id: id.clone(),
                            tool_name: name.clone(),
                        });
                    }
                    _ => {}
                }
            }
            events
        }
        AgentMessage::ToolResult(m) => vec![TuiEvent::ToolDone {
            call_id: m.call_id.clone(),
            text: extract_user_text(&m.content),
            images: extract_display_images(&m.content),
            is_error: m.is_error,
        }],
        AgentMessage::BashExecution(m) => {
            let mut text = String::new();
            if !m.stdout.is_empty() {
                text.push_str(&m.stdout);
            }
            if !m.stderr.is_empty() {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&m.stderr);
            }
            if let Some(code) = m.exit_code {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(&format!("exit code: {code}"));
            }
            vec![TuiEvent::ToolDone {
                call_id: format!("bash-{}", m.id),
                text,
                images: Vec::new(),
                is_error: m.exit_code.is_some_and(|code| code != 0),
            }]
        }
        AgentMessage::CompactionSummary(m) => vec![TuiEvent::SessionCompaction {
            compacted_count: m.compacted_ids.len(),
            tokens_saved: m.tokens_saved,
        }],
        // BranchSummary and Custom messages don't map to conversation blocks.
        AgentMessage::BranchSummary(_) | AgentMessage::Custom(_) => Vec::new(),
    }
}

fn protocol_images_to_display(images: &[ImageData]) -> Vec<DisplayImage> {
    images
        .iter()
        .map(|image| DisplayImage {
            data: image.data.clone(),
            media_type: image.media_type.clone(),
        })
        .collect()
}

fn extract_user_text(content: &[clanker_message::Content]) -> String {
    let mut text = String::new();
    for block in content {
        if let clanker_message::Content::Text { text: block_text } = block {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(block_text);
        }
    }
    text
}

fn extract_display_images(content: &[clanker_message::Content]) -> Vec<DisplayImage> {
    let mut images = Vec::new();
    for block in content {
        if let clanker_message::Content::Image {
            source: clanker_message::ImageSource::Base64 { media_type, data },
        } = block
        {
            images.push(DisplayImage {
                data: data.clone(),
                media_type: media_type.clone(),
            });
        }
    }
    images
}

fn parse_user_input_timestamp(timestamp: &str) -> DateTime<Utc> {
    match DateTime::parse_from_rfc3339(timestamp) {
        Ok(parsed) => parsed.with_timezone(&Utc),
        Err(error) => panic!("daemon user-input timestamp must be RFC3339 UTC: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use clanker_message::SemanticEvent;
    use clanker_message::SemanticToolStatus;

    use super::*;

    fn fixed_timestamp() -> DateTime<Utc> {
        match DateTime::parse_from_rfc3339("2026-04-22T12:34:56Z") {
            Ok(timestamp) => timestamp.with_timezone(&Utc),
            Err(error) => panic!("fixed replay timestamp must parse: {error}"),
        }
    }

    fn user_msg(text: &str) -> clanker_message::transcript::AgentMessage {
        clanker_message::transcript::AgentMessage::User(clanker_message::transcript::UserMessage {
            id: clanker_message::transcript::MessageId::new("u1"),
            content: vec![clanker_message::Content::Text { text: text.to_string() }],
            timestamp: fixed_timestamp(),
        })
    }

    fn assistant_msg(text: &str) -> clanker_message::transcript::AgentMessage {
        clanker_message::transcript::AgentMessage::Assistant(clanker_message::transcript::AssistantMessage {
            id: clanker_message::transcript::MessageId::new("a1"),
            content: vec![clanker_message::Content::Text { text: text.to_string() }],
            model: "test-model".to_string(),
            usage: clanker_message::Usage::default(),
            stop_reason: clanker_message::StopReason::Stop,
            timestamp: fixed_timestamp(),
        })
    }

    #[test]
    fn daemon_event_to_tui_projects_streaming_and_replay_events() {
        let text_event = daemon_event_to_tui_event(&DaemonEvent::TextDelta {
            text: "assistant delta".to_string(),
        });
        assert!(matches!(text_event, Some(TuiEvent::TextDelta(text)) if text == "assistant delta"));

        let timestamp = "2026-04-22T12:34:56Z".to_string();
        let user_event = daemon_event_to_tui_event(&DaemonEvent::UserInput {
            text: "safe replay prompt".to_string(),
            agent_msg_count: 7,
            timestamp: timestamp.clone(),
        });
        assert!(matches!(
            user_event,
            Some(TuiEvent::UserInput { text, agent_msg_count: 7, timestamp: parsed })
                if text == "safe replay prompt" && parsed == parse_user_input_timestamp(&timestamp)
        ));
    }

    #[test]
    fn daemon_event_to_tui_keeps_app_edge_events_direct() {
        for event in [
            DaemonEvent::ConfirmRequest {
                request_id: "r1".to_string(),
                command: "ls".to_string(),
                working_dir: "/".to_string(),
            },
            DaemonEvent::SessionInfo {
                session_id: "s1".to_string(),
                model: "m".to_string(),
                system_prompt_hash: "h".to_string(),
                available_models: Vec::new(),
                active_account: String::new(),
                disabled_tools: Vec::new(),
                auto_test_command: None,
            },
            DaemonEvent::SystemMessage {
                text: "token=[REDACTED]".to_string(),
                is_error: true,
            },
            DaemonEvent::HistoryEnd,
        ] {
            assert!(daemon_event_to_tui_event(&event).is_none());
        }
    }

    #[test]
    fn semantic_event_to_tui_remains_attach_edge_projection() {
        let semantic = SemanticEvent::ToolFinished {
            call_id: "call-parity".to_string(),
            status: SemanticToolStatus::Failed,
            text: "semantic tool output".to_string(),
            images: Vec::new(),
            metadata: clanker_message::SemanticEventMetadata::empty().with_session_id("session-parity"),
        };
        assert!(matches!(
            semantic_event_to_tui_event(&semantic),
            Some(TuiEvent::ToolDone { call_id, text, is_error: true, .. })
                if call_id == "call-parity" && text == "semantic tool output"
        ));
    }

    #[test]
    fn desktop_history_replay_parity_contract_covers_tool_compaction_branch_and_semantics() {
        let user_events = agent_message_to_tui_events(&user_msg("timestamped"));
        assert!(matches!(
            &user_events[0],
            TuiEvent::UserInput { text, timestamp, .. }
                if text == "timestamped" && *timestamp == fixed_timestamp()
        ));

        let tool =
            clanker_message::transcript::AgentMessage::ToolResult(clanker_message::transcript::ToolResultMessage {
                id: clanker_message::transcript::MessageId::new("tool-parity"),
                call_id: "call-parity".to_string(),
                tool_name: "lookup".to_string(),
                content: vec![clanker_message::Content::Text {
                    text: "tool parity output".to_string(),
                }],
                is_error: true,
                details: None,
                timestamp: fixed_timestamp(),
            });
        let tool_events = agent_message_to_tui_events(&tool);
        assert!(matches!(
            &tool_events[0],
            TuiEvent::ToolDone { call_id, text, is_error: true, .. }
                if call_id == "call-parity" && text == "tool parity output"
        ));

        let compaction = clanker_message::transcript::AgentMessage::CompactionSummary(
            clanker_message::transcript::CompactionSummaryMessage {
                id: clanker_message::transcript::MessageId::new("compact-parity"),
                compacted_ids: vec![clanker_message::transcript::MessageId::new("old-1")],
                summary: "compaction context remains metadata".to_string(),
                tokens_saved: 42,
                timestamp: fixed_timestamp(),
            },
        );
        let compaction_events = agent_message_to_tui_events(&compaction);
        assert!(matches!(&compaction_events[0], TuiEvent::SessionCompaction {
            compacted_count: 1,
            tokens_saved: 42
        }));

        let branch = clanker_message::transcript::AgentMessage::BranchSummary(
            clanker_message::transcript::BranchSummaryMessage {
                id: clanker_message::transcript::MessageId::new("branch-parity"),
                from_id: clanker_message::transcript::MessageId::new("user-parity"),
                summary: "branch context remains adapter-owned metadata".to_string(),
                timestamp: fixed_timestamp(),
            },
        );
        assert!(agent_message_to_tui_events(&branch).is_empty());
    }

    #[test]
    fn history_assistant_message_to_tui_events() {
        let events = agent_message_to_tui_events(&assistant_msg("world"));
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], TuiEvent::AgentStart));
        assert!(matches!(&events[1], TuiEvent::ContentBlockStart { is_thinking: false }));
        assert!(matches!(&events[2], TuiEvent::TextDelta(text) if text == "world"));
        assert!(matches!(&events[3], TuiEvent::ContentBlockStop));
    }

    #[test]
    fn history_assistant_with_thinking_and_tool() {
        let msg = clanker_message::transcript::AgentMessage::Assistant(clanker_message::transcript::AssistantMessage {
            id: clanker_message::transcript::MessageId::new("a2"),
            content: vec![
                clanker_message::Content::Thinking {
                    thinking: "let me think".to_string(),
                    signature: String::new(),
                },
                clanker_message::Content::Text {
                    text: "here's my answer".to_string(),
                },
                clanker_message::Content::ToolUse {
                    id: "call_1".to_string(),
                    name: "bash".to_string(),
                    input: serde_json::json!({"command": "ls"}),
                },
            ],
            model: "test".to_string(),
            usage: clanker_message::Usage::default(),
            stop_reason: clanker_message::StopReason::ToolUse,
            timestamp: fixed_timestamp(),
        });

        let events = agent_message_to_tui_events(&msg);
        assert_eq!(events.len(), 9);
        assert!(matches!(&events[0], TuiEvent::AgentStart));
        assert!(matches!(&events[1], TuiEvent::ContentBlockStart { is_thinking: true }));
        assert!(matches!(&events[4], TuiEvent::ContentBlockStart { is_thinking: false }));
        assert!(matches!(&events[7], TuiEvent::ToolCall { tool_name, .. } if tool_name == "bash"));
        assert!(matches!(&events[8], TuiEvent::ToolStart { call_id, .. } if call_id == "call_1"));
    }

    #[test]
    fn history_bash_compaction_branch_and_custom_projection() {
        let bash = clanker_message::transcript::AgentMessage::BashExecution(
            clanker_message::transcript::BashExecutionMessage {
                id: clanker_message::transcript::MessageId::new("be1"),
                command: "ls".to_string(),
                stdout: "file.txt".to_string(),
                stderr: String::new(),
                exit_code: Some(0),
                timestamp: fixed_timestamp(),
            },
        );
        assert!(matches!(&agent_message_to_tui_events(&bash)[0], TuiEvent::ToolDone { text, is_error, .. }
            if text.contains("file.txt") && !is_error));

        let custom = clanker_message::transcript::AgentMessage::Custom(clanker_message::transcript::CustomMessage {
            id: clanker_message::transcript::MessageId::new("cu1"),
            kind: "test".to_string(),
            data: serde_json::json!({}),
            timestamp: fixed_timestamp(),
        });
        assert!(agent_message_to_tui_events(&custom).is_empty());
    }

    #[test]
    fn history_serialization_round_trip() {
        let msg = assistant_msg("round trip test");
        let value = serde_json::to_value(&msg).expect("serialize");
        let restored: clanker_message::transcript::AgentMessage = serde_json::from_value(value).expect("deserialize");
        let events = agent_message_to_tui_events(&restored);
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[2], TuiEvent::TextDelta(text) if text == "round trip test"));
    }
}
