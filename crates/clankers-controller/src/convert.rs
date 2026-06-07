//! Convert AgentEvent → DaemonEvent at the controller boundary.
//!
//! This is the daemon-side equivalent of event_translator.rs in the main crate,
//! but produces protocol DaemonEvents instead of display/TUI events.

use clanker_message::SemanticErrorClass;
use clanker_message::SemanticEvent;
use clanker_message::SemanticEventMetadata;
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
        SemanticEvent::ToolCall { .. }
        | SemanticEvent::ToolStarted { .. }
        | SemanticEvent::ToolOutput { .. }
        | SemanticEvent::ToolProgressUpdate { .. }
        | SemanticEvent::ToolChunk { .. }
        | SemanticEvent::ToolFinished { .. }
        | SemanticEvent::ConfirmationRequested { .. } => semantic_tool_event_to_daemon_event(event),
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

fn semantic_tool_event_to_daemon_event(event: &SemanticEvent) -> Option<DaemonEvent> {
    match event {
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
        SemanticEvent::ToolStarted { call_id, tool_name, .. } => Some(DaemonEvent::ToolStart {
            call_id: call_id.clone(),
            tool_name: tool_name.clone(),
        }),
        SemanticEvent::ToolOutput {
            call_id, text, images, ..
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
        _ => None,
    }
}

pub(crate) fn semantic_error_message_to_daemon_event(
    session_id: &str,
    message: String,
    error_class: SemanticErrorClass,
) -> DaemonEvent {
    let event = SemanticEvent::Error {
        message,
        error_class,
        metadata: SemanticEventMetadata::empty().with_session_id(session_id),
    };
    semantic_event_to_daemon_event(&event).expect("semantic errors always project to daemon system messages")
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

#[cfg(test)]
mod tests {
    use clanker_message::streaming::ContentDelta;
    use clankers_agent::ToolResult;
    use clankers_agent::ToolResultContent;

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
    fn semantic_error_message_projects_through_daemon_system_message() {
        let event = semantic_error_message_to_daemon_event(
            "session-1",
            "Unknown thinking level: bogus".to_string(),
            SemanticErrorClass::InvalidInput,
        );

        assert!(matches!(
            event,
            DaemonEvent::SystemMessage { text, is_error: true }
                if text == "Unknown thinking level: bogus"
        ));
    }

    #[test]
    fn semantic_event_projection_preserves_daemon_and_json_shapes() {
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
}
