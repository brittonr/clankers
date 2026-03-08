//! Agent event handling — routes AgentEvent variants into conversation block state.

use std::time::Instant;

use crate::agent::events::AgentEvent;
use crate::provider::message::Content;
use crate::provider::streaming::ContentDelta;

use super::{ActiveToolExecution, App, AppState, DisplayImage, DisplayMessage, MessageRole};

impl App {
    /// Flush accumulated streaming thinking into the active block
    pub(super) fn flush_streaming_thinking(&mut self) {
        if !self.streaming.thinking.is_empty() {
            let content = std::mem::take(&mut self.streaming.thinking);
            if let Some(ref mut block) = self.conversation.active_block {
                block.responses.push(DisplayMessage {
                    role: MessageRole::Thinking,
                    content,
                    tool_name: None,
                    is_error: false,
                    images: Vec::new(),
                });
            }
        }
    }

    /// Flush accumulated streaming text into the active block
    pub(super) fn flush_streaming_text(&mut self) {
        if !self.streaming.text.is_empty() {
            let content = std::mem::take(&mut self.streaming.text);
            if let Some(ref mut block) = self.conversation.active_block {
                block.responses.push(DisplayMessage {
                    role: MessageRole::Assistant,
                    content,
                    tool_name: None,
                    is_error: false,
                    images: Vec::new(),
                });
            }
        }
    }

    /// Handle an agent event, routing it into the active block
    pub fn handle_agent_event(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::AgentStart => self.on_agent_start(),
            AgentEvent::AgentEnd { .. } => self.on_agent_end(),
            AgentEvent::ContentBlockStart { index, content_block } => {
                self.on_content_block_start(*index, content_block);
            }
            AgentEvent::ContentBlockStop { .. } => self.on_content_block_stop(),
            AgentEvent::MessageUpdate { delta, .. } => self.on_message_update(delta),
            AgentEvent::ToolCall { tool_name, input, .. } => {
                self.on_tool_call(tool_name, input);
            }
            AgentEvent::ToolExecutionStart { call_id, tool_name } => {
                self.on_tool_execution_start(call_id, tool_name);
            }
            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                self.on_tool_execution_update(call_id, partial);
            }
            AgentEvent::ToolProgressUpdate { call_id, progress } => {
                self.streaming.progress_renderer.update(call_id, progress.clone());
            }
            AgentEvent::ToolResultChunk { call_id, chunk } => {
                if chunk.content_type == "text" {
                    self.streaming.outputs.add_text(call_id, &chunk.content);
                }
            }
            AgentEvent::ToolExecutionEnd { call_id, result, is_error, .. } => {
                self.on_tool_execution_end(call_id, result, *is_error);
            }
            AgentEvent::UsageUpdate { cumulative_usage, turn_usage, .. } => {
                self.on_usage_update(cumulative_usage, turn_usage);
            }
            AgentEvent::UserInput { text, agent_msg_count } => {
                self.start_block(text.clone(), *agent_msg_count);
                self.conversation.scroll.scroll_to_bottom();
            }
            AgentEvent::SessionCompaction { compacted_count, tokens_saved } => {
                self.push_system(
                    format!("Auto-compacted {} messages, saved ~{} tokens.", compacted_count, tokens_saved),
                    false,
                );
            }
            _ => {}
        }
    }

    fn on_agent_start(&mut self) {
        self.state = AppState::Streaming;
        self.streaming.text.clear();
        self.streaming.thinking.clear();
        self.streaming.block_index = None;
    }

    fn on_agent_end(&mut self) {
        self.finalize_active_block();
        self.state = AppState::Idle;
        self.conversation.scroll.scroll_to_bottom();
    }

    fn on_content_block_start(&mut self, index: usize, content_block: &Content) {
        match content_block {
            Content::Thinking { .. } => self.flush_streaming_text(),
            Content::Text { .. } => self.flush_streaming_thinking(),
            _ => {
                self.flush_streaming_thinking();
                self.flush_streaming_text();
            }
        }
        self.streaming.block_index = Some(index);
    }

    fn on_content_block_stop(&mut self) {
        self.flush_streaming_thinking();
        self.flush_streaming_text();
        self.streaming.block_index = None;
    }

    fn on_message_update(&mut self, delta: &ContentDelta) {
        match delta {
            ContentDelta::TextDelta { text } => {
                self.streaming.text.push_str(text);
                if self.conversation.scroll.auto_scroll {
                    self.conversation.scroll.scroll_to_bottom();
                }
            }
            ContentDelta::ThinkingDelta { thinking } => {
                self.streaming.thinking.push_str(thinking);
                if self.conversation.scroll.auto_scroll {
                    self.conversation.scroll.scroll_to_bottom();
                }
            }
            _ => {}
        }
    }

    fn on_tool_call(&mut self, tool_name: &str, input: &serde_json::Value) {
        self.flush_streaming_thinking();
        self.flush_streaming_text();
        if let Some(ref mut block) = self.conversation.active_block {
            block.responses.push(DisplayMessage {
                role: MessageRole::ToolCall,
                content: tool_name.to_string(),
                tool_name: Some(tool_name.to_string()),
                is_error: false,
                images: Vec::new(),
            });
        }
        self.track_file_activity(tool_name, input);
    }

    fn on_tool_execution_start(&mut self, call_id: &str, tool_name: &str) {
        self.streaming.active_tools.insert(call_id.to_string(), ActiveToolExecution {
            tool_name: tool_name.to_string(),
            started_at: Instant::now(),
            line_count: 0,
        });
    }

    fn on_tool_execution_update(&mut self, call_id: &str, partial: &crate::tools::ToolResult) {
        let text = partial
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");

        if let Some(active) = self.streaming.active_tools.get_mut(call_id) {
            active.line_count += text.lines().count().max(1);
        }

        self.streaming.outputs.add_text(call_id, &text);

        if let Some(ref mut block) = self.conversation.active_block {
            let found = block
                .responses
                .iter_mut()
                .rev()
                .find(|m| m.role == MessageRole::ToolResult && m.tool_name.as_deref() == Some(call_id));
            if let Some(msg) = found {
                if !msg.content.is_empty() {
                    msg.content.push('\n');
                }
                msg.content.push_str(&text);
            } else {
                block.responses.push(DisplayMessage {
                    role: MessageRole::ToolResult,
                    content: text,
                    tool_name: Some(call_id.to_string()),
                    is_error: false,
                    images: Vec::new(),
                });
            }
        }
        if self.conversation.scroll.auto_scroll {
            self.conversation.scroll.scroll_to_bottom();
        }
    }

    fn on_tool_execution_end(&mut self, call_id: &str, result: &crate::tools::ToolResult, is_error: bool) {
        self.streaming.progress_renderer.remove(call_id);
        self.streaming.active_tools.remove(call_id);
        self.streaming.outputs.remove(call_id);
        if self.streaming.focused_tool.as_deref() == Some(call_id) {
            self.streaming.focused_tool = None;
        }

        let display = result
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let images: Vec<DisplayImage> = result
            .content
            .iter()
            .filter_map(|c| match c {
                crate::tools::ToolResultContent::Image { media_type, data } => Some(DisplayImage {
                    data: data.clone(),
                    media_type: media_type.clone(),
                }),
                _ => None,
            })
            .collect();

        if let Some(ref mut block) = self.conversation.active_block {
            let found = block
                .responses
                .iter_mut()
                .rev()
                .find(|m| m.role == MessageRole::ToolResult && m.tool_name.as_deref() == Some(call_id));
            if let Some(msg) = found {
                msg.content = display;
                msg.is_error = is_error;
                msg.tool_name = None;
                msg.images = images;
            } else {
                block.responses.push(DisplayMessage {
                    role: MessageRole::ToolResult,
                    content: display,
                    tool_name: None,
                    is_error,
                    images,
                });
            }
        }
    }

    fn on_usage_update(
        &mut self,
        cumulative_usage: &crate::provider::Usage,
        turn_usage: &crate::provider::Usage,
    ) {
        self.total_tokens = cumulative_usage.total_tokens();
        if let Some(ref ct) = self.cost_tracker {
            self.total_cost = ct.total_cost();
        }
        if let Some(ref mut block) = self.conversation.active_block {
            block.tokens = block.tokens.saturating_add(turn_usage.total_tokens());
        }
        self.context_gauge.update(
            cumulative_usage.input_tokens,
            cumulative_usage.output_tokens,
            cumulative_usage.cache_creation_input_tokens,
            cumulative_usage.cache_read_input_tokens,
        );
    }

    /// Extract file paths from tool call inputs and record them
    fn track_file_activity(&mut self, tool_name: &str, input: &serde_json::Value) {
        use crate::tui::components::file_activity_panel::FileOp;

        let op = match tool_name {
            "read" => FileOp::Read,
            "edit" => FileOp::Edit,
            "write" => FileOp::Write,
            _ => return,
        };

        if let Some(path) = input.get("path").and_then(|v| v.as_str()) {
            let actual_op = if op == FileOp::Write && !std::path::Path::new(path).exists() {
                FileOp::Create
            } else {
                op
            };
            if let Some(fap) = self.panels.downcast_mut::<crate::tui::components::file_activity_panel::FileActivityPanel>(crate::tui::panel::PanelId::Files) {
                fap.record(path.to_string(), actual_op);
            }
        }
    }
}
