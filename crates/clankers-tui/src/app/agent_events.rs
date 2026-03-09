//! TUI event handling — routes TuiEvent variants into conversation block state.

use std::time::Instant;

use clankers_tui_types::TuiEvent;

use super::ActiveToolExecution;
use super::App;
use super::AppState;
use super::DisplayImage;
use super::DisplayMessage;
use super::MessageRole;

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

    /// Handle a TUI event, routing it into the active block
    pub fn handle_tui_event(&mut self, event: &TuiEvent) {
        match event {
            TuiEvent::AgentStart => self.on_agent_start(),
            TuiEvent::AgentEnd => self.on_agent_end(),
            TuiEvent::ContentBlockStart { is_thinking } => {
                self.on_content_block_start(*is_thinking);
            }
            TuiEvent::ContentBlockStop => self.on_content_block_stop(),
            TuiEvent::TextDelta(text) => self.on_text_delta(text),
            TuiEvent::ThinkingDelta(thinking) => self.on_thinking_delta(thinking),
            TuiEvent::ToolCall {
                tool_name,
                call_id: _,
                input,
            } => {
                self.on_tool_call(tool_name, input);
            }
            TuiEvent::ToolStart { call_id, tool_name } => {
                self.on_tool_execution_start(call_id, tool_name);
            }
            TuiEvent::ToolOutput {
                call_id,
                text,
                images: _,
            } => {
                self.on_tool_execution_update(call_id, text);
            }
            TuiEvent::ToolProgressUpdate { call_id, progress } => {
                self.streaming.progress_renderer.update(call_id, progress.clone());
            }
            TuiEvent::ToolChunk {
                call_id,
                content,
                content_type,
            } => {
                if content_type == "text" {
                    self.streaming.outputs.add_text(call_id, content);
                }
            }
            TuiEvent::ToolDone {
                call_id,
                text,
                images,
                is_error,
            } => {
                self.on_tool_execution_end(call_id, text, images, *is_error);
            }
            TuiEvent::UserInput { text, agent_msg_count } => {
                self.start_block(text.clone(), *agent_msg_count);
                self.conversation.scroll.scroll_to_bottom();
            }
            TuiEvent::SessionCompaction {
                compacted_count,
                tokens_saved,
            } => {
                self.push_system(
                    format!("Auto-compacted {} messages, saved ~{} tokens.", compacted_count, tokens_saved),
                    false,
                );
            }
            TuiEvent::UsageUpdate {
                total_tokens,
                input_tokens,
                output_tokens,
                cache_creation_input_tokens,
                cache_read_input_tokens,
                turn_tokens,
            } => {
                self.on_usage_update(
                    *total_tokens,
                    *input_tokens,
                    *output_tokens,
                    *cache_creation_input_tokens,
                    *cache_read_input_tokens,
                    *turn_tokens,
                );
            }
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

    fn on_content_block_start(&mut self, is_thinking: bool) {
        if is_thinking {
            self.flush_streaming_text();
        } else {
            self.flush_streaming_thinking();
        }
        // block_index is no longer tracked since TuiEvent doesn't carry it
    }

    fn on_content_block_stop(&mut self) {
        self.flush_streaming_thinking();
        self.flush_streaming_text();
        self.streaming.block_index = None;
    }

    fn on_text_delta(&mut self, text: &str) {
        self.streaming.text.push_str(text);
        if self.conversation.scroll.auto_scroll {
            self.conversation.scroll.scroll_to_bottom();
        }
    }

    fn on_thinking_delta(&mut self, thinking: &str) {
        self.streaming.thinking.push_str(thinking);
        if self.conversation.scroll.auto_scroll {
            self.conversation.scroll.scroll_to_bottom();
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
        self.streaming.active_tools.insert(
            call_id.to_string(),
            ActiveToolExecution {
                tool_name: tool_name.to_string(),
                started_at: Instant::now(),
                line_count: 0,
            },
        );
    }

    fn on_tool_execution_update(&mut self, call_id: &str, text: &str) {
        if let Some(active) = self.streaming.active_tools.get_mut(call_id) {
            active.line_count += text.lines().count().max(1);
        }

        self.streaming.outputs.add_text(call_id, text);

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
                msg.content.push_str(text);
            } else {
                block.responses.push(DisplayMessage {
                    role: MessageRole::ToolResult,
                    content: text.to_string(),
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

    fn on_tool_execution_end(&mut self, call_id: &str, text: &str, images: &[DisplayImage], is_error: bool) {
        self.streaming.progress_renderer.remove(call_id);
        self.streaming.active_tools.remove(call_id);
        self.streaming.outputs.remove(call_id);
        if self.streaming.focused_tool.as_deref() == Some(call_id) {
            self.streaming.focused_tool = None;
        }

        if let Some(ref mut block) = self.conversation.active_block {
            let found = block
                .responses
                .iter_mut()
                .rev()
                .find(|m| m.role == MessageRole::ToolResult && m.tool_name.as_deref() == Some(call_id));
            if let Some(msg) = found {
                msg.content = text.to_string();
                msg.is_error = is_error;
                msg.tool_name = None;
                msg.images = images.to_vec();
            } else {
                block.responses.push(DisplayMessage {
                    role: MessageRole::ToolResult,
                    content: text.to_string(),
                    tool_name: None,
                    is_error,
                    images: images.to_vec(),
                });
            }
        }
    }

    fn on_usage_update(
        &mut self,
        total_tokens: usize,
        input_tokens: usize,
        output_tokens: usize,
        cache_creation_input_tokens: usize,
        cache_read_input_tokens: usize,
        turn_tokens: usize,
    ) {
        self.total_tokens = total_tokens;
        if let Some(ref ct) = self.cost_tracker {
            self.total_cost = ct.total_cost();
        }
        if let Some(ref mut block) = self.conversation.active_block {
            block.tokens = block.tokens.saturating_add(turn_tokens);
        }
        self.context_gauge.update(input_tokens, output_tokens, cache_creation_input_tokens, cache_read_input_tokens);
    }

    /// Extract file paths from tool call inputs and record them
    fn track_file_activity(&mut self, tool_name: &str, input: &serde_json::Value) {
        use crate::components::file_activity_panel::FileOp;

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
            if let Some(fap) =
                self.panels.downcast_mut::<crate::components::file_activity_panel::FileActivityPanel>(
                    crate::panel::PanelId::Files,
                )
            {
                fap.record(path.to_string(), actual_op);
            }
        }
    }
}
