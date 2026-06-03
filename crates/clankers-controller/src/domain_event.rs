//! Shared semantic event compatibility adapter for controller projection.
//!
//! `ControllerDomainEvent` is kept as the controller-local compatibility name
//! while migration converges on [`clanker_message::SemanticEvent`]. The type is
//! a direct alias, so transport and display code project from the reusable
//! semantic event contract instead of a controller-private event model.

use clanker_message::SemanticEvent;
use clanker_message::SemanticImage;
use clankers_agent::ToolResultContent;
use clankers_agent::events::AgentEvent;

pub(crate) type ControllerDomainEvent = SemanticEvent;
pub(crate) type DomainImage = SemanticImage;

pub(crate) fn agent_event_to_domain_event(event: &AgentEvent) -> Option<ControllerDomainEvent> {
    event.to_semantic_event()
}

#[allow(dead_code)]
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
    use clanker_message::SemanticToolStatus;
    use clanker_message::streaming::ContentDelta;
    use clankers_agent::ToolResult;
    use clankers_agent::events::AgentEvent;

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
            Some(ControllerDomainEvent::AssistantDelta {
                text: "hello".to_string(),
                metadata: clanker_message::SemanticEventMetadata::empty().with("source", "agent"),
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
            Some(ControllerDomainEvent::ToolFinished {
                call_id: "call-1".to_string(),
                status: SemanticToolStatus::Failed,
                text: "line 1\nline 2".to_string(),
                images: vec![DomainImage {
                    data: "base64".to_string(),
                    media_type: "image/png".to_string(),
                }],
                metadata: clanker_message::SemanticEventMetadata::empty().with("source", "agent"),
            })
        );
    }

    #[test]
    fn ignores_agent_internal_context_events() {
        let event = AgentEvent::TurnStart { index: 3 };
        assert_eq!(agent_event_to_domain_event(&event), None);
    }
}
