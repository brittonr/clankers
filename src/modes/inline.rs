//! Inline scrollback output mode.
//!
//! Renders agent events as styled terminal output that accumulates in
//! scrollback, using `rat-inline` for markdown rendering, frame diffing,
//! and reconciliation.

use std::collections::HashMap;
use std::io::Write;
use std::sync::Arc;

use rat_inline::{InlineMarkdown, InlineRenderer, InlineText, InlineView};
use ratatui::style::{Modifier, Style};

use crate::agent::builder::AgentBuilder;
use crate::agent::events::AgentEvent;
use crate::error::Result;
use crate::provider::streaming::ContentDelta;

/// Options controlling inline output behaviour.
#[derive(Debug, Clone, Default)]
pub struct InlineOptions {
    /// Output file (None = stdout)
    pub output_file: Option<String>,
    /// Show token usage stats at the end
    pub show_stats: bool,
    /// Show tool calls and results
    pub show_tools: bool,
    /// Extended thinking configuration
    pub thinking: Option<crate::provider::ThinkingConfig>,
}

// ---------------------------------------------------------------------------
// State model
// ---------------------------------------------------------------------------

/// A single content block within a turn.
#[derive(Debug, Clone)]
enum Block {
    Text { content: String },
    Thinking { content: String },
}

/// A tool call and its accumulated output.
#[derive(Debug, Clone)]
struct ToolCallState {
    tool_name: String,
    summary: String,
    output: String,
    is_error: bool,
    finished: bool,
}

/// Accumulated state for the inline renderer.
///
/// Each event mutates this; `build_view` reads it to produce the full
/// declarative view tree. The renderer's reconciler + diff engine turns
/// that into minimal ANSI output.
#[derive(Debug)]
pub struct InlineState {
    /// Per-turn content blocks (text / thinking).
    turns: Vec<Vec<Block>>,
    /// Tool calls keyed by call_id.
    tools: HashMap<String, ToolCallState>,
    /// Ordered tool call IDs (insertion order for display).
    tool_order: Vec<String>,
    /// Which turn each tool call belongs to.
    tool_turn: HashMap<String, usize>,
    /// Current turn index.
    current_turn: usize,
    /// Final usage stats (set on AgentEnd / UsageUpdate).
    usage: Option<UsageStats>,
    /// Show tool calls in the view.
    show_tools: bool,
    /// Show usage stats in the view.
    show_stats: bool,
}

#[derive(Debug, Clone)]
struct UsageStats {
    input_tokens: usize,
    output_tokens: usize,
    cache_read: usize,
    cache_write: usize,
}

impl InlineState {
    pub fn new(show_tools: bool, show_stats: bool) -> Self {
        Self {
            turns: vec![Vec::new()],
            tools: HashMap::new(),
            tool_order: Vec::new(),
            tool_turn: HashMap::new(),
            current_turn: 0,
            usage: None,
            show_tools,
            show_stats,
        }
    }

    /// Apply a single agent event, mutating accumulated state.
    pub fn apply(&mut self, event: &AgentEvent) {
        match event {
            AgentEvent::TurnStart { index } => {
                let idx = *index as usize;
                self.current_turn = idx;
                while self.turns.len() <= idx {
                    self.turns.push(Vec::new());
                }
            }

            AgentEvent::ContentBlockStart { content_block, .. } => {
                use crate::provider::message::Content;
                match content_block {
                    Content::Thinking { .. } => {
                        self.current_blocks_mut().push(Block::Thinking {
                            content: String::new(),
                        });
                    }
                    Content::Text { text } if !text.is_empty() => {
                        self.current_blocks_mut().push(Block::Text {
                            content: text.clone(),
                        });
                    }
                    _ => {}
                }
            }

            AgentEvent::MessageUpdate { delta, .. } => match delta {
                ContentDelta::TextDelta { text } => {
                    self.ensure_text_block();
                    if let Some(Block::Text { content }) = self.current_blocks_mut().last_mut() {
                        content.push_str(text);
                    }
                }
                ContentDelta::ThinkingDelta { thinking } => {
                    self.ensure_thinking_block();
                    if let Some(Block::Thinking { content }) =
                        self.current_blocks_mut().last_mut()
                    {
                        content.push_str(thinking);
                    }
                }
                _ => {}
            },

            AgentEvent::ToolCall {
                tool_name,
                call_id,
                input,
            } => {
                let summary = tool_call_summary(tool_name, input);
                self.tools.insert(
                    call_id.clone(),
                    ToolCallState {
                        tool_name: tool_name.clone(),
                        summary,
                        output: String::new(),
                        is_error: false,
                        finished: false,
                    },
                );
                self.tool_order.push(call_id.clone());
                self.tool_turn.insert(call_id.clone(), self.current_turn);
            }

            AgentEvent::ToolExecutionUpdate { call_id, partial } => {
                if let Some(tc) = self.tools.get_mut(call_id) {
                    let text = extract_tool_text(partial);
                    if !text.is_empty() {
                        tc.output.push_str(&text);
                    }
                }
            }

            AgentEvent::ToolExecutionEnd {
                call_id,
                result,
                is_error,
            } => {
                if let Some(tc) = self.tools.get_mut(call_id) {
                    tc.is_error = *is_error;
                    tc.finished = true;
                    // If there was no streaming output, use the final result
                    if tc.output.is_empty() {
                        tc.output = extract_tool_text(result);
                    }
                }
            }

            AgentEvent::UsageUpdate {
                cumulative_usage, ..
            } => {
                self.usage = Some(UsageStats {
                    input_tokens: cumulative_usage.input_tokens,
                    output_tokens: cumulative_usage.output_tokens,
                    cache_read: cumulative_usage.cache_read_input_tokens,
                    cache_write: cumulative_usage.cache_creation_input_tokens,
                });
            }

            _ => {}
        }
    }

    /// Build the full declarative view tree from accumulated state.
    pub fn build_view(&self) -> InlineView {
        let mut view = InlineView::new();

        for (turn_idx, blocks) in self.turns.iter().enumerate() {
            // Turn separator (after the first turn)
            if turn_idx > 0 {
                let has_content = !blocks.is_empty()
                    || self.tool_order.iter().any(|id| {
                        self.tool_turn.get(id).copied() == Some(turn_idx)
                    });
                if has_content {
                    view = view.keyed(
                        format!("sep-{turn_idx}"),
                        InlineText::new("─".repeat(40)).style(
                            Style::default().add_modifier(Modifier::DIM),
                        ),
                    );
                }
            }

            // Content blocks
            for (block_idx, block) in blocks.iter().enumerate() {
                match block {
                    Block::Text { content } if !content.is_empty() => {
                        view = view.keyed(
                            format!("msg-{turn_idx}-{block_idx}"),
                            InlineMarkdown::new(content),
                        );
                    }
                    Block::Thinking { content } if !content.is_empty() => {
                        let display = format!("Thinking... {content}");
                        view = view.keyed(
                            format!("think-{turn_idx}-{block_idx}"),
                            InlineText::new(display).style(
                                Style::default()
                                    .add_modifier(Modifier::DIM)
                                    .add_modifier(Modifier::ITALIC),
                            ),
                        );
                    }
                    _ => {}
                }
            }

            // Tool calls for this turn
            if self.show_tools {
                for call_id in &self.tool_order {
                    if self.tool_turn.get(call_id).copied() != Some(turn_idx) {
                        continue;
                    }
                    if let Some(tc) = self.tools.get(call_id) {
                        // Header
                        let header = format!("⚡ {}: {}", tc.tool_name, tc.summary);
                        view = view.keyed(
                            format!("tool-{call_id}"),
                            InlineText::new(header).style(
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                        );

                        // Output (truncated)
                        if !tc.output.is_empty() {
                            let display = truncate_tool_output(&tc.output, 20);
                            let style = if tc.is_error {
                                Style::default()
                                    .fg(ratatui::style::Color::Red)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().add_modifier(Modifier::DIM)
                            };
                            view = view.keyed(
                                format!("tool-out-{call_id}"),
                                InlineText::new(display).style(style),
                            );
                        }
                    }
                }
            }
        }

        // Usage stats footer
        if self.show_stats
            && let Some(ref usage) = self.usage
        {
            let mut line = format!(
                "tokens: {}in / {}out",
                usage.input_tokens, usage.output_tokens,
            );
            if usage.cache_read > 0 || usage.cache_write > 0 {
                use std::fmt::Write;
                let _ = write!(
                    line,
                    "  cache: {}read / {}write",
                    usage.cache_read, usage.cache_write,
                );
            }
            view = view.keyed(
                "usage",
                InlineText::new(line).style(
                    Style::default().add_modifier(Modifier::DIM),
                ),
            );
        }

        view
    }

    // -- helpers --

    fn current_blocks_mut(&mut self) -> &mut Vec<Block> {
        while self.turns.len() <= self.current_turn {
            self.turns.push(Vec::new());
        }
        &mut self.turns[self.current_turn]
    }

    fn ensure_text_block(&mut self) {
        let blocks = self.current_blocks_mut();
        if !matches!(blocks.last(), Some(Block::Text { .. })) {
            blocks.push(Block::Text {
                content: String::new(),
            });
        }
    }

    fn ensure_thinking_block(&mut self) {
        let blocks = self.current_blocks_mut();
        if !matches!(blocks.last(), Some(Block::Thinking { .. })) {
            blocks.push(Block::Thinking {
                content: String::new(),
            });
        }
    }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the inline rendering mode.
#[cfg_attr(
    dylint_lib = "tigerstyle",
    allow(function_length, reason = "sequential setup/dispatch logic")
)]
pub async fn run_inline_with_options(
    prompt: &str,
    provider: Arc<dyn crate::provider::Provider>,
    tools: Vec<Arc<dyn crate::tools::Tool>>,
    settings: crate::config::settings::Settings,
    model: String,
    system_prompt: String,
    opts: InlineOptions,
) -> Result<()> {
    let mut builder =
        AgentBuilder::new(provider, settings.clone(), model, system_prompt).with_tools(tools);
    if let Some(thinking) = opts.thinking.clone() {
        builder = builder.with_thinking(thinking);
    }
    if let Some(caps) = &settings.default_capabilities {
        let gate = Arc::new(crate::capability_gate::UcanCapabilityGate::new(
            caps.clone(),
        ));
        builder = builder.with_capability_gate(gate);
    }
    let mut agent = builder.build();
    let mut rx = agent.subscribe();

    let show_tools = opts.show_tools;
    let show_stats = opts.show_stats;
    let output_file = opts.output_file.clone();

    // Spawn the agent on a Send-compatible task; render on the current
    // thread because InlineRenderer contains non-Send dyn trait objects.
    let prompt_owned = prompt.to_string();
    let agent_handle = tokio::spawn(async move {
        agent.prompt(&prompt_owned).await
    });

    // Detect terminal width, fall back to 80
    let width = crossterm::terminal::size()
        .map(|(w, _)| w)
        .unwrap_or(80);

    let mut renderer = InlineRenderer::new(width);
    let mut state = InlineState::new(show_tools, show_stats);

    let mut writer: Box<dyn Write> = if let Some(ref path) = output_file {
        match std::fs::File::create(path) {
            Ok(f) => Box::new(std::io::BufWriter::new(f)),
            Err(e) => {
                eprintln!("clankers: failed to open output file '{}': {}", path, e);
                return Ok(());
            }
        }
    } else {
        Box::new(std::io::stdout())
    };

    while let Ok(event) = rx.recv().await {
        let is_end = matches!(event, AgentEvent::AgentEnd { .. });

        state.apply(&event);
        let view = state.build_view();
        renderer.rebuild(view);
        let output = renderer.render();
        if !output.is_empty() {
            writer.write_all(&output).ok();
            writer.flush().ok();
        }

        if is_end {
            break;
        }
    }

    // Reset terminal style and print final newline
    writer.write_all(b"\x1b[0m\n").ok();
    writer.flush().ok();

    // Wait for agent to finish
    if let Ok(result) = agent_handle.await {
        result?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Produce a short summary for a tool call header.
fn tool_call_summary(tool_name: &str, input: &serde_json::Value) -> String {
    match tool_name {
        "bash" | "Bash" => input
            .get("command")
            .and_then(|v| v.as_str())
            .map(|s| truncate_str(s, 60).to_string())
            .unwrap_or_default(),
        "read" | "Read" => input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "write" | "Write" => input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "edit" | "Edit" => input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => {
            let s = input.to_string();
            truncate_str(&s, 60).to_string()
        }
    }
}

fn truncate_str(s: &str, max: usize) -> &str {
    if s.len() <= max {
        s
    } else {
        // Find a safe char boundary
        let mut end = max;
        while end > 0 && !s.is_char_boundary(end) {
            end -= 1;
        }
        &s[..end]
    }
}

fn truncate_tool_output(s: &str, max_lines: usize) -> String {
    let lines: Vec<&str> = s.lines().collect();
    if lines.len() <= max_lines {
        s.to_string()
    } else {
        let kept: Vec<&str> = lines[..max_lines].to_vec();
        format!(
            "{}\n... ({} more lines)",
            kept.join("\n"),
            lines.len() - max_lines
        )
    }
}

fn extract_tool_text(result: &crate::agent::tool::ToolResult) -> String {
    use crate::tools::ToolResultContent;
    result
        .content
        .iter()
        .filter_map(|c| {
            if let ToolResultContent::Text { text } = c {
                Some(text.as_str())
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::events::AgentEvent;
    use crate::agent::tool::ToolResult;
    use crate::provider::streaming::ContentDelta;
    use serde_json::json;

    fn text_delta(text: &str) -> AgentEvent {
        AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::TextDelta {
                text: text.to_string(),
            },
        }
    }

    fn thinking_delta(text: &str) -> AgentEvent {
        AgentEvent::MessageUpdate {
            index: 0,
            delta: ContentDelta::ThinkingDelta {
                thinking: text.to_string(),
            },
        }
    }

    fn tool_call(name: &str, id: &str, input: serde_json::Value) -> AgentEvent {
        AgentEvent::ToolCall {
            tool_name: name.to_string(),
            call_id: id.to_string(),
            input,
        }
    }

    fn tool_end(id: &str, text: &str, is_error: bool) -> AgentEvent {
        AgentEvent::ToolExecutionEnd {
            call_id: id.to_string(),
            result: ToolResult::text(text),
            is_error,
        }
    }

    // -- Task 6.1: Unit test InlineState::apply --

    #[test]
    fn apply_text_deltas_accumulate() {
        let mut state = InlineState::new(true, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&text_delta("Hello "));
        state.apply(&text_delta("world"));

        assert_eq!(state.turns.len(), 1);
        assert_eq!(state.turns[0].len(), 1);
        match &state.turns[0][0] {
            Block::Text { content } => assert_eq!(content, "Hello world"),
            _ => panic!("expected text block"),
        }
    }

    #[test]
    fn apply_thinking_deltas_accumulate() {
        let mut state = InlineState::new(true, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&thinking_delta("Let me "));
        state.apply(&thinking_delta("think..."));

        assert_eq!(state.turns[0].len(), 1);
        match &state.turns[0][0] {
            Block::Thinking { content } => assert_eq!(content, "Let me think..."),
            _ => panic!("expected thinking block"),
        }
    }

    #[test]
    fn apply_tool_calls_tracked() {
        let mut state = InlineState::new(true, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&tool_call("bash", "call-1", json!({"command": "ls"})));
        state.apply(&tool_end("call-1", "file.rs", false));

        assert_eq!(state.tools.len(), 1);
        let tc = state.tools.get("call-1").unwrap();
        assert_eq!(tc.tool_name, "bash");
        assert_eq!(tc.output, "file.rs");
        assert!(!tc.is_error);
        assert!(tc.finished);
    }

    #[test]
    fn apply_tool_error_flagged() {
        let mut state = InlineState::new(true, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&tool_call("bash", "call-2", json!({"command": "false"})));
        state.apply(&tool_end("call-2", "exit code 1", true));

        let tc = state.tools.get("call-2").unwrap();
        assert!(tc.is_error);
    }

    #[test]
    fn apply_multi_turn() {
        let mut state = InlineState::new(true, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&text_delta("Turn 0"));
        state.apply(&AgentEvent::TurnStart { index: 1 });
        state.apply(&text_delta("Turn 1"));

        assert_eq!(state.turns.len(), 2);
        match &state.turns[0][0] {
            Block::Text { content } => assert_eq!(content, "Turn 0"),
            _ => panic!("expected text"),
        }
        match &state.turns[1][0] {
            Block::Text { content } => assert_eq!(content, "Turn 1"),
            _ => panic!("expected text"),
        }
    }

    // -- Task 6.2: Unit test InlineState::build_view --

    #[test]
    fn build_view_produces_correct_node_count() {
        let mut state = InlineState::new(true, false);

        // Turn 0: text + tool
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&text_delta("Hello"));
        state.apply(&tool_call("bash", "c1", json!({"command": "ls"})));
        state.apply(&tool_end("c1", "output", false));

        // Turn 1: text
        state.apply(&AgentEvent::TurnStart { index: 1 });
        state.apply(&text_delta("Done"));

        let view = state.build_view();
        let (tree, widgets) = view.build();

        // Turn 0: msg-0-0, tool-c1, tool-out-c1
        // Separator: sep-1
        // Turn 1: msg-1-0
        assert_eq!(tree.len(), 5);
        assert_eq!(widgets.len(), 5);

        // Verify keys
        assert_eq!(tree.nodes[0].key.as_ref().unwrap().0, "msg-0-0");
        assert_eq!(tree.nodes[1].key.as_ref().unwrap().0, "tool-c1");
        assert_eq!(tree.nodes[2].key.as_ref().unwrap().0, "tool-out-c1");
        assert_eq!(tree.nodes[3].key.as_ref().unwrap().0, "sep-1");
        assert_eq!(tree.nodes[4].key.as_ref().unwrap().0, "msg-1-0");
    }

    #[test]
    fn build_view_thinking_nodes() {
        let mut state = InlineState::new(false, false);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&thinking_delta("hmm"));
        state.apply(&text_delta("answer"));

        let (tree, _) = state.build_view().build();
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.nodes[0].key.as_ref().unwrap().0, "think-0-0");
        assert_eq!(tree.nodes[1].key.as_ref().unwrap().0, "msg-0-1");
    }

    #[test]
    fn build_view_usage_stats() {
        let mut state = InlineState::new(false, true);
        state.apply(&AgentEvent::TurnStart { index: 0 });
        state.apply(&text_delta("hi"));
        state.apply(&AgentEvent::UsageUpdate {
            turn_usage: crate::provider::Usage {
                input_tokens: 10,
                output_tokens: 5,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
            cumulative_usage: crate::provider::Usage {
                input_tokens: 100,
                output_tokens: 50,
                cache_read_input_tokens: 0,
                cache_creation_input_tokens: 0,
            },
        });

        let (tree, _) = state.build_view().build();
        // msg-0-0 + usage
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.nodes[1].key.as_ref().unwrap().0, "usage");
    }
}
