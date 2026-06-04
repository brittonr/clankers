//! AgentEvent enum (lifecycle, tool, message events)

use chrono::DateTime;
use chrono::Utc;
use clanker_message::transcript::AgentMessage;
use clanker_message::transcript::AssistantMessage;
use clanker_message::Content;
use clanker_message::transcript::MessageId;
use clanker_message::SemanticErrorClass;
use clanker_message::SemanticEvent;
use clanker_message::SemanticEventMetadata;
use clanker_message::SemanticImage;
use clanker_message::SemanticToolStatus;
use clanker_message::transcript::ToolResultMessage;
use clanker_message::Usage;
use clanker_message::streaming::StreamDelta;
use serde_json::Value;

use crate::tool::ToolResult;

/// All lifecycle events emitted by the agent during execution.
/// Consumed by TUI, JSON mode, print mode, plugins, etc.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    // Session lifecycle
    SessionStart {
        session_id: String,
    },
    SessionShutdown {
        session_id: String,
    },

    // Agent lifecycle
    AgentStart,
    AgentEnd {
        messages: Vec<AgentMessage>,
    },

    // Turn lifecycle
    TurnStart {
        index: u32,
    },
    TurnEnd {
        index: u32,
        message: AssistantMessage,
        tool_results: Vec<ToolResultMessage>,
    },

    // Message streaming
    MessageStart {
        message: AgentMessage,
    },
    /// A new content block has started streaming
    ContentBlockStart {
        index: usize,
        content_block: Content,
    },
    /// Incremental delta for a content block
    MessageUpdate {
        index: usize,
        delta: StreamDelta,
    },
    /// A content block has finished streaming
    ContentBlockStop {
        index: usize,
    },
    MessageEnd {
        message: AgentMessage,
    },

    // Tool events
    ToolCall {
        tool_name: String,
        call_id: String,
        input: Value,
    },
    ToolExecutionStart {
        call_id: String,
        tool_name: String,
    },
    ToolExecutionUpdate {
        call_id: String,
        partial: ToolResult,
    },
    ToolExecutionEnd {
        call_id: String,
        result: ToolResult,
        is_error: bool,
    },
    ToolResultEvent {
        tool_name: String,
        call_id: String,
        content: Vec<Content>,
        details: Option<Value>,
    },

    // Structured progress and result streaming
    /// Structured progress update from a tool
    ToolProgressUpdate {
        call_id: String,
        progress: crate::tool::progress::ToolProgress,
    },
    /// Result chunk streamed from a tool
    ToolResultChunk {
        call_id: String,
        chunk: crate::tool::progress::ResultChunk,
    },

    // Context
    BeforeAgentStart {
        prompt: String,
        system_prompt: String,
    },
    Context {
        messages: Vec<AgentMessage>,
    },

    // Session events
    SessionBranch {
        from_id: MessageId,
        branch_id: MessageId,
    },
    SessionCompaction {
        compacted_count: usize,
        tokens_saved: usize,
    },
    SessionCompactionSummary {
        summary: String,
    },

    // Model events
    ModelChange {
        from: String,
        to: String,
        reason: String,
    },

    // Input events
    UserInput {
        text: String,
        /// Number of agent messages *before* this user message was appended
        agent_msg_count: usize,
        /// Canonical timestamp of the stored user message.
        timestamp: DateTime<Utc>,
    },
    UserCancel,
    SystemMessage {
        message: String,
    },

    // Cost tracking
    UsageUpdate {
        turn_usage: Usage,
        cumulative_usage: Usage,
    },

    // Process monitoring events
    ProcessSpawn {
        pid: u32,
        meta: clankers_procmon::ProcessMeta,
    },
    ProcessSample {
        pid: u32,
        cpu_percent: f32,
        rss_bytes: u64,
        children: Vec<u32>,
    },
    ProcessExit {
        pid: u32,
        exit_code: Option<i32>,
        wall_time: std::time::Duration,
        peak_rss: u64,
    },
}

impl AgentEvent {
    /// Convert this agent event into the shared semantic event stream.
    #[must_use]
    pub fn to_semantic_event(&self) -> Option<SemanticEvent> {
        match self {
            Self::AgentStart => Some(SemanticEvent::AgentStart {
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::AgentEnd { .. } => Some(SemanticEvent::AgentEnd {
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::ContentBlockStart { content_block, .. } => Some(SemanticEvent::ContentBlockStart {
                is_thinking: matches!(content_block, Content::Thinking { .. }),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::ContentBlockStop { .. } => Some(SemanticEvent::ContentBlockStop {
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::MessageUpdate { delta, .. } => stream_delta_to_semantic_event(delta),
            Self::ToolCall {
                tool_name,
                call_id,
                input,
            } => Some(SemanticEvent::ToolCall {
                tool_name: tool_name.clone(),
                call_id: call_id.clone(),
                input: input.clone(),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::ToolExecutionStart { call_id, tool_name } => Some(SemanticEvent::ToolStarted {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::ToolExecutionUpdate { call_id, partial } => {
                let (text, images) = tool_result_content_to_semantic_parts(&partial.content);
                Some(SemanticEvent::ToolOutput {
                    call_id: call_id.clone(),
                    text,
                    images,
                    metadata: SemanticEventMetadata::empty().with("source", "agent"),
                })
            }
            Self::ToolExecutionEnd {
                call_id,
                result,
                is_error,
            } => {
                let (text, images) = tool_result_content_to_semantic_parts(&result.content);
                Some(SemanticEvent::ToolFinished {
                    call_id: call_id.clone(),
                    status: if *is_error {
                        SemanticToolStatus::Failed
                    } else {
                        SemanticToolStatus::Succeeded
                    },
                    text,
                    images,
                    metadata: SemanticEventMetadata::empty().with("source", "agent"),
                })
            }
            Self::ToolProgressUpdate { call_id, progress } => Some(SemanticEvent::ToolProgressUpdate {
                call_id: call_id.clone(),
                message: progress.message.clone(),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::ToolResultChunk { call_id, chunk } => Some(SemanticEvent::ToolChunk {
                call_id: call_id.clone(),
                content: chunk.content.clone(),
                content_type: chunk.content_type.clone(),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::UserInput {
                text,
                agent_msg_count,
                timestamp,
            } => Some(SemanticEvent::UserInput {
                text: text.clone(),
                agent_msg_count: *agent_msg_count,
                timestamp_rfc3339: timestamp.to_rfc3339(),
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::SessionCompaction {
                compacted_count,
                tokens_saved,
            } => Some(SemanticEvent::SessionCompaction {
                compacted_count: *compacted_count,
                tokens_saved: *tokens_saved,
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::UsageUpdate { cumulative_usage, .. } => Some(SemanticEvent::UsageUpdated {
                input_tokens: cumulative_usage.input_tokens as u64,
                output_tokens: cumulative_usage.output_tokens as u64,
                cache_read_tokens: cumulative_usage.cache_read_input_tokens as u64,
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::UserCancel => Some(SemanticEvent::Error {
                message: "user cancelled".to_string(),
                error_class: SemanticErrorClass::Session,
                metadata: SemanticEventMetadata::empty().with("source", "agent"),
            }),
            Self::SessionShutdown { session_id } => Some(SemanticEvent::Shutdown {
                metadata: SemanticEventMetadata::empty().with("source", "agent").with_session_id(session_id.clone()),
            }),
            _ => None,
        }
    }

    /// String tag for plugin event matching.
    ///
    /// Returns the snake_case identifier that plugin manifests use
    /// in their `"events"` array to subscribe to this event type.
    pub fn event_kind(&self) -> &'static str {
        match self {
            Self::SessionStart { .. } => "session_start",
            Self::SessionShutdown { .. } => "session_end",
            Self::AgentStart => "agent_start",
            Self::AgentEnd { .. } => "agent_end",
            Self::TurnStart { .. } => "turn_start",
            Self::TurnEnd { .. } => "turn_end",
            Self::ToolCall { .. } => "tool_call",
            Self::ToolExecutionStart { .. } => "tool_execution_start",
            Self::ToolExecutionEnd { .. } => "tool_result",
            Self::MessageUpdate { .. } => "message_update",
            Self::UserInput { .. } => "user_input",
            Self::ModelChange { .. } => "model_change",
            Self::UsageUpdate { .. } => "usage_update",
            Self::SessionBranch { .. } => "session_branch",
            Self::SessionCompaction { .. } => "session_compaction",
            Self::SessionCompactionSummary { .. } => "session_compaction_summary",
            Self::UserCancel => "user_cancel",
            Self::SystemMessage { .. } => "system_message",
            _ => "",
        }
    }
}

fn stream_delta_to_semantic_event(delta: &StreamDelta) -> Option<SemanticEvent> {
    match delta {
        clanker_message::ContentDelta::TextDelta { text } => Some(SemanticEvent::AssistantDelta {
            text: text.clone(),
            metadata: SemanticEventMetadata::empty().with("source", "agent"),
        }),
        clanker_message::ContentDelta::ThinkingDelta { thinking } => Some(SemanticEvent::ThinkingDelta {
            text: thinking.clone(),
            metadata: SemanticEventMetadata::empty().with("source", "agent"),
        }),
        clanker_message::ContentDelta::InputJsonDelta { .. } | clanker_message::ContentDelta::SignatureDelta { .. } => {
            None
        }
    }
}

fn tool_result_content_to_semantic_parts(content: &[crate::tool::ToolResultContent]) -> (String, Vec<SemanticImage>) {
    let mut text = String::new();
    let mut images = Vec::new();
    for item in content {
        match item {
            crate::tool::ToolResultContent::Text { text: fragment } => {
                if !text.is_empty() {
                    text.push('\n');
                }
                text.push_str(fragment);
            }
            crate::tool::ToolResultContent::Image { media_type, data } => images.push(SemanticImage {
                data: data.clone(),
                media_type: media_type.clone(),
            }),
        }
    }
    (text, images)
}

/// Convert a `ProcessEvent` from the procmon crate into an `AgentEvent`.
#[cfg(test)]
mod tests {
    use clanker_message::ContentDelta;
    use clanker_message::SemanticToolStatus;

    use super::*;
    use crate::tool::ToolResult;
    use crate::tool::ToolResultContent;

    #[test]
    fn agent_event_projects_core_semantic_order() {
        let events = vec![
            AgentEvent::AgentStart,
            AgentEvent::MessageUpdate {
                index: 0,
                delta: ContentDelta::ThinkingDelta {
                    thinking: "thinking".to_string(),
                },
            },
            AgentEvent::MessageUpdate {
                index: 0,
                delta: ContentDelta::TextDelta {
                    text: "answer".to_string(),
                },
            },
            AgentEvent::ToolExecutionStart {
                call_id: "call-1".to_string(),
                tool_name: "bash".to_string(),
            },
            AgentEvent::ToolExecutionEnd {
                call_id: "call-1".to_string(),
                result: ToolResult {
                    content: vec![ToolResultContent::Text { text: "ok".to_string() }],
                    details: None,
                    full_output_path: None,
                    is_error: false,
                },
                is_error: false,
            },
            AgentEvent::AgentEnd { messages: Vec::new() },
        ];
        let semantic: Vec<_> = events
            .iter()
            .map(|event| event.to_semantic_event().expect("fixture events map to semantic"))
            .collect();
        let kinds: Vec<_> = semantic.iter().map(SemanticEvent::kind).collect();
        assert_eq!(kinds, vec![
            "agent_start",
            "thinking_delta",
            "assistant_delta",
            "tool_started",
            "tool_finished",
            "agent_end"
        ]);
        assert!(matches!(&semantic[4], SemanticEvent::ToolFinished {
            status: SemanticToolStatus::Succeeded,
            ..
        }));
    }
}

pub fn process_event_to_agent(pe: clankers_procmon::ProcessEvent) -> AgentEvent {
    match pe {
        clankers_procmon::ProcessEvent::Spawn { pid, meta } => AgentEvent::ProcessSpawn { pid, meta },
        clankers_procmon::ProcessEvent::Sample {
            pid,
            cpu_percent,
            rss_bytes,
            children,
        } => AgentEvent::ProcessSample {
            pid,
            cpu_percent,
            rss_bytes,
            children,
        },
        clankers_procmon::ProcessEvent::Exit {
            pid,
            exit_code,
            wall_time,
            peak_rss,
        } => AgentEvent::ProcessExit {
            pid,
            exit_code,
            wall_time,
            peak_rss,
        },
    }
}
