//! Convert AgentEvent → DaemonEvent at the controller boundary.
//!
//! This is the daemon-side equivalent of event_translator.rs in the main crate,
//! but produces protocol DaemonEvents instead of TuiEvents.

use chrono::DateTime;
use chrono::Utc;
use clanker_message::SemanticEvent;
use clanker_message::SemanticToolStatus;
use clankers_agent::events::AgentEvent;
use clankers_protocol::event::DaemonEvent;
use clankers_protocol::types::ImageData;

use crate::domain_event::DomainImage;
use crate::domain_event::agent_event_to_domain_event;

/// Translate an AgentEvent into a DaemonEvent (or None for events clients
/// don't need, like Context, BeforeAgentStart, TurnStart, etc.).
pub fn agent_event_to_daemon_event(event: &AgentEvent) -> Option<DaemonEvent> {
    agent_event_to_domain_event(event).and_then(|event| semantic_event_to_daemon_event(&event))
}

pub fn semantic_event_to_daemon_event(event: &SemanticEvent) -> Option<DaemonEvent> {
    match event {
        SemanticEvent::AgentStart { .. } => Some(DaemonEvent::AgentStart),
        SemanticEvent::AgentEnd { .. } => Some(DaemonEvent::AgentEnd),
        SemanticEvent::ContentBlockStart { is_thinking, .. } => Some(DaemonEvent::ContentBlockStart {
            is_thinking: *is_thinking,
        }),
        SemanticEvent::ContentBlockStop { .. } => Some(DaemonEvent::ContentBlockStop),
        SemanticEvent::AssistantDelta { text, .. } => Some(DaemonEvent::TextDelta { text: text.clone() }),
        SemanticEvent::ThinkingDelta { text, .. } => Some(DaemonEvent::ThinkingDelta { text: text.clone() }),
        SemanticEvent::ToolCall {
            tool_name,
            call_id,
            input,
            ..
        } => Some(DaemonEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        SemanticEvent::ToolStarted {
            call_id, tool_name, ..
        } => Some(DaemonEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        SemanticEvent::ToolOutput {
            call_id,
            text,
            images,
            ..
        } => Some(DaemonEvent::ToolOutput {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images.iter().cloned().map(domain_image_to_protocol_image).collect(),
        }),
        SemanticEvent::ToolProgressUpdate { call_id, message, .. } => Some(DaemonEvent::ToolProgressUpdate {
            call_id: call_id.clone(),
            progress: serde_json::json!({ "message": message }),
        }),
        SemanticEvent::ToolChunk {
            call_id,
            content,
            content_type,
            ..
        } => Some(DaemonEvent::ToolChunk {
            call_id: call_id.clone(),
            content: content.clone(),
            content_type: content_type.clone(),
        }),
        SemanticEvent::ToolFinished {
            call_id,
            status,
            text,
            images,
            ..
        } => Some(DaemonEvent::ToolDone {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images.iter().cloned().map(domain_image_to_protocol_image).collect(),
            is_error: matches!(status, SemanticToolStatus::Failed | SemanticToolStatus::Denied),
        }),
        SemanticEvent::ConfirmationRequested { request, .. } => Some(DaemonEvent::ConfirmRequest {
            request_id: request.request_id.clone(),
            command: request.summary.clone(),
            working_dir: request.working_dir.clone().unwrap_or_default(),
        }),
        SemanticEvent::UsageUpdated {
            input_tokens,
            output_tokens,
            cache_read_tokens,
            ..
        } => Some(DaemonEvent::UsageUpdate {
            input_tokens: *input_tokens,
            output_tokens: *output_tokens,
            cache_read: *cache_read_tokens,
            model: String::new(),
        }),
        SemanticEvent::Error { message, .. } => Some(DaemonEvent::SystemMessage {
            text: message.clone(),
            is_error: true,
        }),
        SemanticEvent::UserInput {
            text,
            agent_msg_count,
            timestamp_rfc3339,
            ..
        } => Some(DaemonEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
            timestamp: timestamp_rfc3339.clone(),
        }),
        SemanticEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
            ..
        } => Some(DaemonEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        SemanticEvent::PromptAccepted { .. } | SemanticEvent::Completed { .. } | SemanticEvent::Shutdown { .. } => None,
    }
}

pub fn semantic_event_to_tui_event(event: &SemanticEvent) -> Option<clanker_tui_types::TuiEvent> {
    semantic_event_to_daemon_event(event).and_then(|event| daemon_event_to_tui_event(&event))
}

pub fn semantic_event_to_json_value(event: &SemanticEvent) -> serde_json::Value {
    serde_json::to_value(event).unwrap_or_else(|_| serde_json::json!({ "type": "serialization_error" }))
}

fn domain_image_to_protocol_image(image: DomainImage) -> ImageData {
    ImageData {
        data: image.data,
        media_type: image.media_type,
    }
}

/// Convert DaemonEvent into a TuiEvent for the client side.
///
/// This is the inverse of `agent_event_to_daemon_event` — the TUI client
/// calls this to produce TuiEvents from the socket stream.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(
        function_length,
        reason = "sequential setup/dispatch logic — splitting would fragment readability"
    )
)]
pub fn daemon_event_to_tui_event(event: &DaemonEvent) -> Option<clanker_tui_types::TuiEvent> {
    match event {
        DaemonEvent::AgentStart => Some(clanker_tui_types::TuiEvent::AgentStart),
        DaemonEvent::AgentEnd => Some(clanker_tui_types::TuiEvent::AgentEnd),

        DaemonEvent::ContentBlockStart { is_thinking } => Some(clanker_tui_types::TuiEvent::ContentBlockStart {
            is_thinking: *is_thinking,
        }),
        DaemonEvent::ContentBlockStop => Some(clanker_tui_types::TuiEvent::ContentBlockStop),

        DaemonEvent::TextDelta { text } => Some(clanker_tui_types::TuiEvent::TextDelta(text.clone())),
        DaemonEvent::ThinkingDelta { text } => Some(clanker_tui_types::TuiEvent::ThinkingDelta(text.clone())),

        DaemonEvent::ToolCall {
            tool_name,
            call_id,
            input,
        } => Some(clanker_tui_types::TuiEvent::ToolCall {
            tool_name: tool_name.clone(),
            call_id: call_id.clone(),
            input: input.clone(),
        }),
        DaemonEvent::ToolStart { call_id, tool_name } => Some(clanker_tui_types::TuiEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        DaemonEvent::ToolOutput { call_id, text, images } => Some(clanker_tui_types::TuiEvent::ToolOutput {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images
                .iter()
                .map(|i| clanker_tui_types::DisplayImage {
                    data: i.data.clone(),
                    media_type: i.media_type.clone(),
                })
                .collect(),
        }),
        DaemonEvent::ToolProgressUpdate { call_id, progress } => {
            // Best-effort conversion — structured progress may not round-trip perfectly
            // ToolProgress contains non-serializable Instant, so we reconstruct
            let _ = progress;
            Some(clanker_tui_types::TuiEvent::ToolProgressUpdate {
                call_id: call_id.clone(),
                progress: clanker_tui_types::ToolProgress {
                    kind: clanker_tui_types::ProgressKind::Phase {
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
        } => Some(clanker_tui_types::TuiEvent::ToolChunk {
            call_id: call_id.clone(),
            content: content.clone(),
            content_type: content_type.clone(),
        }),
        DaemonEvent::ToolDone {
            call_id,
            text,
            images,
            is_error,
        } => Some(clanker_tui_types::TuiEvent::ToolDone {
            call_id: call_id.clone(),
            text: text.clone(),
            images: images
                .iter()
                .map(|i| clanker_tui_types::DisplayImage {
                    data: i.data.clone(),
                    media_type: i.media_type.clone(),
                })
                .collect(),
            is_error: *is_error,
        }),

        DaemonEvent::UserInput {
            text,
            agent_msg_count,
            timestamp,
        } => Some(clanker_tui_types::TuiEvent::UserInput {
            text: text.clone(),
            agent_msg_count: *agent_msg_count,
            timestamp: parse_user_input_timestamp(timestamp),
        }),
        DaemonEvent::SessionCompaction {
            compacted_count,
            tokens_saved,
        } => Some(clanker_tui_types::TuiEvent::SessionCompaction {
            compacted_count: *compacted_count,
            tokens_saved: *tokens_saved,
        }),
        DaemonEvent::UsageUpdate {
            input_tokens,
            output_tokens,
            cache_read,
            ..
        } => Some(clanker_tui_types::TuiEvent::UsageUpdate {
            total_tokens: usize::try_from(*input_tokens + *output_tokens).unwrap_or(usize::MAX),
            input_tokens: usize::try_from(*input_tokens).unwrap_or(usize::MAX),
            output_tokens: usize::try_from(*output_tokens).unwrap_or(usize::MAX),
            cache_creation_input_tokens: 0,
            cache_read_input_tokens: usize::try_from(*cache_read).unwrap_or(usize::MAX),
            turn_tokens: 0,
        }),

        // Events that don't map to TuiEvent — handled by the client directly
        _ => None,
    }
}

// ── History replay conversion ───────────────────────────────────────────────

/// Convert a stored `AgentMessage` into TUI events for history replay.
///
/// Returns the sequence of `TuiEvent`s that reconstruct this message in the
/// TUI's block-based conversation view. Replay keeps the active block open
/// across assistant and tool-result messages until the next user prompt or the
/// explicit history-end marker finalises it.
pub fn agent_message_to_tui_events(msg: &clanker_message::AgentMessage) -> Vec<clanker_tui_types::TuiEvent> {
    use clanker_message::AgentMessage;
    use clanker_message::Content;
    use clanker_tui_types::TuiEvent;

    match msg {
        AgentMessage::User(m) => {
            let text = extract_user_text(&m.content);
            vec![TuiEvent::UserInput {
                text,
                agent_msg_count: 0,
                timestamp: m.timestamp,
            }]
        }

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
                    // Images and ToolResults inside assistant blocks are rare;
                    // skip for replay.
                    _ => {}
                }
            }

            events
        }

        AgentMessage::ToolResult(m) => {
            let text = extract_user_text(&m.content);
            vec![TuiEvent::ToolDone {
                call_id: m.call_id.clone(),
                text,
                images: extract_display_images(&m.content),
                is_error: m.is_error,
            }]
        }

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
                images: vec![],
                is_error: m.exit_code.is_some_and(|c| c != 0),
            }]
        }

        AgentMessage::CompactionSummary(m) => {
            vec![TuiEvent::SessionCompaction {
                compacted_count: m.compacted_ids.len(),
                tokens_saved: m.tokens_saved,
            }]
        }

        // BranchSummary and Custom messages don't map to conversation blocks.
        AgentMessage::BranchSummary(_) | AgentMessage::Custom(_) => vec![],
    }
}

/// Extract display text from content blocks.
fn extract_user_text(content: &[clanker_message::Content]) -> String {
    let mut text = String::new();
    for block in content {
        if let clanker_message::Content::Text { text: t } = block {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(t);
        }
    }
    text
}

/// Extract images from content blocks as `DisplayImage`.
fn extract_display_images(content: &[clanker_message::Content]) -> Vec<clanker_tui_types::DisplayImage> {
    let mut images = Vec::new();
    for block in content {
        if let clanker_message::Content::Image {
            source: clanker_message::ImageSource::Base64 { media_type, data },
        } = block
        {
            images.push(clanker_tui_types::DisplayImage {
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
    use clankers_agent::ToolResult;
    use clankers_agent::ToolResultContent;
    use clankers_provider::streaming::ContentDelta;

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
    fn test_user_input_converts_with_timestamp() {
        const AGENT_MESSAGE_COUNT: usize = 3;
        let timestamp = chrono::Utc::now();
        let event = AgentEvent::UserInput {
            text: "hello".to_string(),
            agent_msg_count: AGENT_MESSAGE_COUNT,
            timestamp,
        };
        let result = agent_event_to_daemon_event(&event);
        assert!(matches!(
            result,
            Some(DaemonEvent::UserInput {
                text,
                agent_msg_count: AGENT_MESSAGE_COUNT,
                timestamp: converted_timestamp,
            }) if text == "hello" && converted_timestamp == timestamp.to_rfc3339()
        ));
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
        assert!(matches!(result, Some(clanker_tui_types::TuiEvent::AgentStart)));
    }

    #[test]
    fn test_daemon_to_tui_text_delta() {
        let event = DaemonEvent::TextDelta {
            text: "hello".to_string(),
        };
        let result = daemon_event_to_tui_event(&event);
        assert!(matches!(result, Some(clanker_tui_types::TuiEvent::TextDelta(t)) if t == "hello"));
    }

    #[test]
    fn test_daemon_user_input_converts_with_timestamp() {
        const AGENT_MESSAGE_COUNT: usize = 3;
        let timestamp = "2026-04-22T12:34:56Z".to_string();
        let result = daemon_event_to_tui_event(&DaemonEvent::UserInput {
            text: "hello".to_string(),
            agent_msg_count: AGENT_MESSAGE_COUNT,
            timestamp: timestamp.clone(),
        });
        assert!(matches!(
            result,
            Some(clanker_tui_types::TuiEvent::UserInput {
                text,
                agent_msg_count: AGENT_MESSAGE_COUNT,
                timestamp: parsed_timestamp,
            }) if text == "hello" && parsed_timestamp == parse_user_input_timestamp(&timestamp)
        ));
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
                available_models: Vec::new(),
                active_account: String::new(),
                disabled_tools: Vec::new(),
                auto_test_command: None,
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
    fn daemon_event_translation_kit_preserves_streaming_replay_and_app_edge_events() {
        let text_event = daemon_event_to_tui_event(&DaemonEvent::TextDelta {
            text: "assistant delta".to_string(),
        });
        assert!(matches!(text_event, Some(clanker_tui_types::TuiEvent::TextDelta(text)) if text == "assistant delta"));

        let timestamp = "2026-04-22T12:34:56Z".to_string();
        let user_event = daemon_event_to_tui_event(&DaemonEvent::UserInput {
            text: "safe replay prompt".to_string(),
            agent_msg_count: 7,
            timestamp: timestamp.clone(),
        });
        assert!(matches!(
            user_event,
            Some(clanker_tui_types::TuiEvent::UserInput { text, agent_msg_count: 7, timestamp: parsed })
                if text == "safe replay prompt" && parsed == parse_user_input_timestamp(&timestamp)
        ));

        let app_edge_event = DaemonEvent::SystemMessage {
            text: "token=[REDACTED]".to_string(),
            is_error: true,
        };
        assert!(daemon_event_to_tui_event(&app_edge_event).is_none());

        let branch = clanker_message::AgentMessage::BranchSummary(clanker_message::BranchSummaryMessage {
            id: clanker_message::MessageId::new("bs-kit"),
            from_id: clanker_message::MessageId::new("m-kit"),
            summary: "branch summaries stay app-edge replay metadata".to_string(),
            timestamp: fixed_timestamp(),
        });
        assert!(agent_message_to_tui_events(&branch).is_empty());
    }

    #[test]
    fn semantic_event_projection_preserves_daemon_tui_and_json_shapes() {
        let metadata = clanker_message::SemanticEventMetadata::empty()
            .with_session_id("session-1")
            .with_prompt_id("prompt-1")
            .with("authorization", "Bearer SECRET_TOKEN");
        let event = SemanticEvent::ToolFinished {
            call_id: "call-1".to_string(),
            status: SemanticToolStatus::Failed,
            text: "tool failed".to_string(),
            images: vec![clanker_message::SemanticImage {
                data: "base64".to_string(),
                media_type: "image/png".to_string(),
            }],
            metadata,
        };

        let daemon = semantic_event_to_daemon_event(&event).expect("tool maps to daemon event");
        assert!(matches!(
            &daemon,
            DaemonEvent::ToolDone {
                call_id,
                text,
                images,
                is_error: true,
            } if call_id == "call-1" && text == "tool failed" && images.len() == 1
        ));
        let tui = semantic_event_to_tui_event(&event).expect("tool maps to tui event");
        assert!(matches!(
            tui,
            clanker_tui_types::TuiEvent::ToolDone {
                call_id,
                text,
                is_error: true,
                ..
            } if call_id == "call-1" && text == "tool failed"
        ));
        let json = semantic_event_to_json_value(&event);
        let json_text = serde_json::to_string(&json).expect("json value serializes");
        assert_eq!(json["type"], "tool_finished");
        assert!(!json_text.contains("SECRET_TOKEN"));
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

        let (text, images) = crate::domain_event::tool_content_to_domain_parts(&content);
        assert_eq!(text, "line1\nline2");
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].media_type, "image/png");
    }

    // ── History replay tests ────────────────────────────────────────

    fn fixed_timestamp() -> chrono::DateTime<chrono::Utc> {
        match chrono::DateTime::parse_from_rfc3339("2026-04-22T12:34:56Z") {
            Ok(timestamp) => timestamp.with_timezone(&chrono::Utc),
            Err(error) => panic!("fixed replay timestamp must parse: {error}"),
        }
    }

    fn user_msg(text: &str) -> clanker_message::AgentMessage {
        clanker_message::AgentMessage::User(clanker_message::UserMessage {
            id: clanker_message::MessageId::new("u1"),
            content: vec![clanker_message::Content::Text { text: text.to_string() }],
            timestamp: fixed_timestamp(),
        })
    }

    fn assistant_msg(text: &str) -> clanker_message::AgentMessage {
        clanker_message::AgentMessage::Assistant(clanker_message::AssistantMessage {
            id: clanker_message::MessageId::new("a1"),
            content: vec![clanker_message::Content::Text { text: text.to_string() }],
            model: "test-model".to_string(),
            usage: clanker_message::Usage::default(),
            stop_reason: clanker_message::StopReason::Stop,
            timestamp: fixed_timestamp(),
        })
    }

    #[test]
    fn history_user_message_to_tui_events() {
        let message = user_msg("hello");
        let events = agent_message_to_tui_events(&message);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            clanker_tui_types::TuiEvent::UserInput {
                text,
                timestamp,
                ..
            } if text == "hello" && *timestamp == fixed_timestamp()
        ));
    }

    #[test]
    fn history_assistant_message_to_tui_events() {
        let events = agent_message_to_tui_events(&assistant_msg("world"));
        // AgentStart, ContentBlockStart, TextDelta, ContentBlockStop
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[0], clanker_tui_types::TuiEvent::AgentStart));
        assert!(matches!(&events[1], clanker_tui_types::TuiEvent::ContentBlockStart { is_thinking: false }));
        assert!(matches!(&events[2], clanker_tui_types::TuiEvent::TextDelta(t) if t == "world"));
        assert!(matches!(&events[3], clanker_tui_types::TuiEvent::ContentBlockStop));
    }

    #[test]
    fn history_assistant_with_thinking_and_tool() {
        let msg = clanker_message::AgentMessage::Assistant(clanker_message::AssistantMessage {
            id: clanker_message::MessageId::new("a2"),
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
            timestamp: chrono::Utc::now(),
        });

        let events = agent_message_to_tui_events(&msg);
        // AgentStart, think block (3), text block (3), tool call + start (2)
        assert_eq!(events.len(), 9);
        assert!(matches!(&events[0], clanker_tui_types::TuiEvent::AgentStart));
        assert!(matches!(&events[1], clanker_tui_types::TuiEvent::ContentBlockStart { is_thinking: true }));
        assert!(matches!(&events[4], clanker_tui_types::TuiEvent::ContentBlockStart { is_thinking: false }));
        assert!(matches!(&events[7], clanker_tui_types::TuiEvent::ToolCall { tool_name, .. } if tool_name == "bash"));
        assert!(matches!(&events[8], clanker_tui_types::TuiEvent::ToolStart { call_id, .. } if call_id == "call_1"));
    }

    #[test]
    fn history_tool_result_to_tui_events() {
        let msg = clanker_message::AgentMessage::ToolResult(clanker_message::ToolResultMessage {
            id: clanker_message::MessageId::new("tr1"),
            call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            content: vec![clanker_message::Content::Text {
                text: "output".to_string(),
            }],
            is_error: false,
            details: None,
            timestamp: chrono::Utc::now(),
        });

        let events = agent_message_to_tui_events(&msg);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], clanker_tui_types::TuiEvent::ToolDone { call_id, text, is_error, .. }
            if call_id == "call_1" && text == "output" && !is_error));
    }

    #[test]
    fn history_bash_execution_to_tui_events() {
        let msg = clanker_message::AgentMessage::BashExecution(clanker_message::BashExecutionMessage {
            id: clanker_message::MessageId::new("be1"),
            command: "ls".to_string(),
            stdout: "file.txt".to_string(),
            stderr: String::new(),
            exit_code: Some(0),
            timestamp: chrono::Utc::now(),
        });

        let events = agent_message_to_tui_events(&msg);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], clanker_tui_types::TuiEvent::ToolDone { text, is_error, .. }
            if text.contains("file.txt") && !is_error));
    }

    #[test]
    fn history_compaction_to_tui_events() {
        let msg = clanker_message::AgentMessage::CompactionSummary(clanker_message::CompactionSummaryMessage {
            id: clanker_message::MessageId::new("cs1"),
            compacted_ids: vec![
                clanker_message::MessageId::new("m1"),
                clanker_message::MessageId::new("m2"),
            ],
            summary: "compacted".to_string(),
            tokens_saved: 1000,
            timestamp: chrono::Utc::now(),
        });

        let events = agent_message_to_tui_events(&msg);
        assert_eq!(events.len(), 1);
        assert!(matches!(&events[0], clanker_tui_types::TuiEvent::SessionCompaction {
            compacted_count: 2,
            tokens_saved: 1000,
        }));
    }

    #[test]
    fn history_branch_and_custom_produce_no_events() {
        let branch = clanker_message::AgentMessage::BranchSummary(clanker_message::BranchSummaryMessage {
            id: clanker_message::MessageId::new("bs1"),
            from_id: clanker_message::MessageId::new("m1"),
            summary: "branched".to_string(),
            timestamp: chrono::Utc::now(),
        });
        assert!(agent_message_to_tui_events(&branch).is_empty());

        let custom = clanker_message::AgentMessage::Custom(clanker_message::CustomMessage {
            id: clanker_message::MessageId::new("cu1"),
            kind: "test".to_string(),
            data: serde_json::json!({}),
            timestamp: chrono::Utc::now(),
        });
        assert!(agent_message_to_tui_events(&custom).is_empty());
    }

    #[test]
    fn history_serialization_round_trip() {
        // Verify that AgentMessage survives serde_json::to_value → from_value
        let msg = assistant_msg("round trip test");
        let value = serde_json::to_value(&msg).expect("serialize");
        let restored: clanker_message::AgentMessage = serde_json::from_value(value).expect("deserialize");
        let events = agent_message_to_tui_events(&restored);
        assert_eq!(events.len(), 4);
        assert!(matches!(&events[2], clanker_tui_types::TuiEvent::TextDelta(t) if t == "round trip test"));
    }
}
