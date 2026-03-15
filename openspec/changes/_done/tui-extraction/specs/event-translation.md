# Event Translation — AgentEvent → TuiEvent

## Overview

The TUI crate defines its own event type (`TuiEvent`) that contains only
display-relevant data. The main crate translates `AgentEvent` variants into
`TuiEvent` before forwarding to the TUI's event channel. This prevents the
TUI from depending on agent, provider, tool, or session types.

## TuiEvent Definition

Lives in `crates/clankers-tui/src/events.rs`.

```rust
use clankers_tui_types::*;

/// Events the TUI can receive, from any source.
#[derive(Debug, Clone)]
pub enum TuiEvent {
    // ── Agent lifecycle ──────────────────────────────
    /// Agent started processing a prompt
    AgentStart,
    /// Agent finished processing
    AgentEnd,

    // ── Streaming ────────────────────────────────────
    /// A new content block started
    ContentBlockStart { is_thinking: bool },
    /// Incremental text delta
    TextDelta(String),
    /// Incremental thinking delta
    ThinkingDelta(String),
    /// Content block finished
    ContentBlockStop,

    // ── Tool events ──────────────────────────────────
    /// Tool was called by the model
    ToolCall {
        tool_name: String,
        call_id: String,
        input_preview: String,
    },
    /// Tool started executing
    ToolStart {
        call_id: String,
        tool_name: String,
    },
    /// Tool produced output (streaming)
    ToolOutput {
        call_id: String,
        text: String,
        is_error: bool,
    },
    /// Tool finished
    ToolDone {
        call_id: String,
        result_text: String,
        is_error: bool,
    },
    /// Tool structured progress update
    ToolProgress {
        call_id: String,
        progress: ToolProgressData,
    },
    /// Tool result chunk (streaming result accumulation)
    ToolChunk {
        call_id: String,
        text: String,
    },
    /// Tool result with rich content (images, structured data)
    ToolResult {
        call_id: String,
        tool_name: String,
        content_text: String,
        images: Vec<DisplayImage>,
    },

    // ── Subagent events ──────────────────────────────
    /// Forwarded from SubagentEvent (already a shared type)
    Subagent(SubagentEvent),

    // ── Process monitor events ───────────────────────
    ProcessSpawn {
        pid: u32,
        name: String,
        command: String,
    },
    ProcessSample {
        pid: u32,
        cpu_percent: f32,
        rss_bytes: u64,
    },
    ProcessExit {
        pid: u32,
        exit_code: Option<i32>,
        wall_time: std::time::Duration,
        peak_rss: u64,
    },

    // ── Session events ───────────────────────────────
    SessionStart { session_id: String },
    SessionBranch { from_block: usize, new_block: usize },
    CostUpdate { total_cost: f64, total_tokens: usize },
    ModelChange { from: String, to: String, reason: String },
    UserInput { text: String },

    // ── Terminal input (from crossterm) ───────────────
    Key(crossterm::event::KeyEvent),
    Paste(String),
    Resize(u16, u16),
    Mouse { col: u16, row: u16, kind: MouseEventKind },
    Tick,
}

/// Mouse event kind (simplified from crossterm)
#[derive(Debug, Clone, Copy)]
pub enum MouseEventKind {
    Click(MouseButton),
    ScrollUp,
    ScrollDown,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left, Right, Middle,
}
```

## Translation Function

Lives in `src/event_translator.rs` in the main crate.

```rust
use crate::agent::events::AgentEvent;
use crate::provider::message::Content;
use crate::provider::streaming::{ContentDelta, StreamDelta};
use clankers_tui::TuiEvent;
use clankers_tui_types::DisplayImage;

/// Translate an AgentEvent into zero or one TuiEvent.
///
/// Returns `None` for events the TUI doesn't need (e.g., `Context`,
/// `BeforeAgentStart`, `TurnStart`, `TurnEnd`).
pub fn translate(event: &AgentEvent) -> Option<TuiEvent> {
    match event {
        // Lifecycle
        AgentEvent::AgentStart => Some(TuiEvent::AgentStart),
        AgentEvent::AgentEnd { .. } => Some(TuiEvent::AgentEnd),

        // Streaming
        AgentEvent::ContentBlockStart { content_block, .. } => {
            let is_thinking = matches!(content_block, Content::Thinking { .. });
            Some(TuiEvent::ContentBlockStart { is_thinking })
        }
        AgentEvent::ContentBlockStop { .. } => Some(TuiEvent::ContentBlockStop),
        AgentEvent::MessageUpdate { delta, .. } => match &delta.delta {
            ContentDelta::Text(s) => Some(TuiEvent::TextDelta(s.clone())),
            ContentDelta::Thinking(s) => Some(TuiEvent::ThinkingDelta(s.clone())),
            _ => None,
        },

        // Tools
        AgentEvent::ToolCall { tool_name, call_id, input } => {
            let input_preview = serde_json::to_string(input)
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect();
            Some(TuiEvent::ToolCall {
                tool_name: tool_name.clone(),
                call_id: call_id.clone(),
                input_preview,
            })
        }
        AgentEvent::ToolExecutionStart { call_id, tool_name } => {
            Some(TuiEvent::ToolStart {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
            })
        }
        AgentEvent::ToolExecutionUpdate { call_id, partial } => {
            Some(TuiEvent::ToolOutput {
                call_id: call_id.clone(),
                text: partial.text().unwrap_or_default().to_string(),
                is_error: partial.is_error(),
            })
        }
        AgentEvent::ToolExecutionEnd { call_id, result, is_error } => {
            Some(TuiEvent::ToolDone {
                call_id: call_id.clone(),
                result_text: result.text().unwrap_or_default().to_string(),
                is_error: *is_error,
            })
        }
        AgentEvent::ToolProgressUpdate { call_id, progress } => {
            Some(TuiEvent::ToolProgress {
                call_id: call_id.clone(),
                progress: progress.into(), // From<&ToolProgress> for ToolProgressData
            })
        }
        AgentEvent::ToolResultChunk { call_id, chunk } => {
            Some(TuiEvent::ToolChunk {
                call_id: call_id.clone(),
                text: chunk.content.clone(),
            })
        }
        AgentEvent::ToolResultEvent { tool_name, call_id, content, .. } => {
            let mut text = String::new();
            let mut images = Vec::new();
            for c in content {
                match c {
                    Content::Text(t) => text.push_str(t),
                    Content::Image { data, media_type } => {
                        images.push(DisplayImage {
                            data: data.clone(),
                            media_type: media_type.clone(),
                        });
                    }
                    _ => {}
                }
            }
            Some(TuiEvent::ToolResult {
                call_id: call_id.clone(),
                tool_name: tool_name.clone(),
                content_text: text,
                images,
            })
        }

        // Process monitor
        AgentEvent::ProcessSpawn { pid, meta } => {
            Some(TuiEvent::ProcessSpawn {
                pid: *pid,
                name: meta.tool_name.clone(),
                command: meta.command.clone(),
            })
        }
        AgentEvent::ProcessSample { pid, cpu_percent, rss_bytes, .. } => {
            Some(TuiEvent::ProcessSample {
                pid: *pid,
                cpu_percent: *cpu_percent,
                rss_bytes: *rss_bytes,
            })
        }
        AgentEvent::ProcessExit { pid, exit_code, wall_time, peak_rss } => {
            Some(TuiEvent::ProcessExit {
                pid: *pid,
                exit_code: *exit_code,
                wall_time: *wall_time,
                peak_rss: *peak_rss,
            })
        }

        // Session
        AgentEvent::SessionStart { session_id } => {
            Some(TuiEvent::SessionStart { session_id: session_id.clone() })
        }
        AgentEvent::ModelChange { from, to, reason } => {
            Some(TuiEvent::ModelChange {
                from: from.clone(),
                to: to.clone(),
                reason: reason.clone(),
            })
        }
        AgentEvent::UsageUpdate { cumulative_usage, .. } => {
            Some(TuiEvent::CostUpdate {
                total_cost: cumulative_usage.total_cost(),
                total_tokens: cumulative_usage.total_tokens(),
            })
        }
        AgentEvent::UserInput { text, .. } => {
            Some(TuiEvent::UserInput { text: text.clone() })
        }

        // Events the TUI doesn't need
        AgentEvent::SessionShutdown { .. }
        | AgentEvent::TurnStart { .. }
        | AgentEvent::TurnEnd { .. }
        | AgentEvent::MessageStart { .. }
        | AgentEvent::MessageEnd { .. }
        | AgentEvent::BeforeAgentStart { .. }
        | AgentEvent::Context { .. }
        | AgentEvent::SessionCompaction { .. }
        | AgentEvent::UserCancel => None,

        // SessionBranch uses MessageId internally — extract the block ID
        AgentEvent::SessionBranch { from_id, branch_id } => {
            // Block ID extraction depends on session internals;
            // the main crate resolves MessageId → block index before sending
            None // handled separately by the main crate
        }
    }
}
```

## Translator Task

The main crate spawns a task that bridges the agent's broadcast channel to
the TUI's mpsc channel:

```rust
// In src/modes/interactive.rs (or src/event_translator.rs)

fn spawn_event_translator(
    mut agent_rx: broadcast::Receiver<AgentEvent>,
    tui_tx: mpsc::Sender<TuiEvent>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            match agent_rx.recv().await {
                Ok(event) => {
                    if let Some(tui_event) = translate(&event) {
                        if tui_tx.send(tui_event).is_err() {
                            break; // TUI closed
                        }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Event translator lagged, skipped {n} events");
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}
```

## Current vs. New Flow

### Current (agent_events.rs in TUI module)

```
AgentEvent (broadcast) → EventLoopRunner.try_recv()
                        → app.handle_agent_event(&event)
                        → match on AgentEvent variants
                        → mutate App fields directly
```

`handle_agent_event` imports `AgentEvent`, `Content`, `ContentDelta`,
`ToolResult`, `Usage` — 9 external types.

### New (translator + TuiEvent)

```
AgentEvent (broadcast) → translator task
                        → translate(&event) → Option<TuiEvent>
                        → mpsc::send(tui_event)
                        → EventLoopRunner.recv()
                        → app.handle_tui_event(&event)
                        → match on TuiEvent variants
                        → mutate App fields directly
```

`handle_tui_event` imports only `TuiEvent` — all types are TUI-owned.

### What changes in `agent_events.rs`

The file stays in the TUI crate but the `impl App` block changes:

```rust
// Before
pub fn handle_agent_event(&mut self, event: &AgentEvent) {
    match event {
        AgentEvent::AgentStart => self.on_agent_start(),
        AgentEvent::ContentBlockStart { index, content_block } => { ... }
        AgentEvent::MessageUpdate { delta, .. } => {
            match &delta.delta {
                ContentDelta::Text(s) => self.streaming.text.push_str(s),
                ...
            }
        }
        ...
    }
}

// After
pub fn handle_tui_event(&mut self, event: &TuiEvent) {
    match event {
        TuiEvent::AgentStart => self.on_agent_start(),
        TuiEvent::ContentBlockStart { is_thinking } => { ... }
        TuiEvent::TextDelta(s) => self.streaming.text.push_str(s),
        TuiEvent::ThinkingDelta(s) => self.streaming.thinking.push_str(s),
        TuiEvent::ToolCall { tool_name, call_id, input_preview } => { ... }
        ...
    }
}
```

The internal handler methods (`on_agent_start`, `on_agent_end`,
`flush_streaming_text`, etc.) stay unchanged — they only touch `App` fields.
