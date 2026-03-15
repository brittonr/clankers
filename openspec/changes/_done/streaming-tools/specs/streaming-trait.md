# Streaming Trait — ToolContext Extensions

## Overview

The `Tool` trait signature remains unchanged:
```rust
#[async_trait]
pub trait Tool: Send + Sync {
    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult;
}
```

Streaming support is added via **new methods on ToolContext**, not a new trait.
This is additive and non-breaking. Tools opt-in to streaming by calling the new
methods. The existing `emit_progress(&self, text: &str)` remains unchanged for
backward compatibility.

## ToolContext Extensions

### Current ToolContext (unchanged core)

```rust
pub struct ToolContext {
    /// Unique identifier for this tool call
    pub call_id: String,
    /// Cancellation token
    pub signal: CancellationToken,
    /// Event bus sender (optional)
    event_tx: Option<broadcast::Sender<AgentEvent>>,
    /// Throttle state (new field, private)
    throttle_state: Arc<Mutex<ThrottleState>>,
}
```

### New Throttle State

```rust
struct ThrottleState {
    /// Last time we emitted a structured progress event for this call_id
    last_progress_emit: Option<Instant>,
    /// Minimum interval between progress emissions (default: 100ms)
    min_interval: Duration,
}

impl Default for ThrottleState {
    fn default() -> Self {
        Self {
            last_progress_emit: None,
            min_interval: Duration::from_millis(100),
        }
    }
}
```

### New Methods

```rust
impl ToolContext {
    /// Emit text-only progress (existing, unchanged)
    pub fn emit_progress(&self, text: &str) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(AgentEvent::ToolExecutionUpdate {
                call_id: self.call_id.clone(),
                partial: ToolResult::text(text),
            });
        }
    }

    /// Emit structured progress update
    ///
    /// Throttled to max 10 updates/sec (100ms interval) per call_id.
    /// If called more frequently, the event is silently dropped.
    pub fn emit_structured_progress(&self, progress: ToolProgress) {
        let mut state = self.throttle_state.lock().unwrap();

        // Check throttle
        if let Some(last) = state.last_progress_emit {
            if last.elapsed() < state.min_interval {
                // Drop this event (throttled)
                return;
            }
        }

        // Update throttle state
        state.last_progress_emit = Some(Instant::now());
        drop(state);

        // Emit event
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(AgentEvent::ToolProgressUpdate {
                call_id: self.call_id.clone(),
                progress,
            });
        }
    }

    /// Emit a result chunk
    ///
    /// NOT throttled — result chunks are streamed as fast as produced.
    /// Back-pressure is handled by the event bus ring buffer (drop-oldest).
    pub fn emit_result_chunk(&self, chunk: ToolResultChunk) {
        if let Some(ref tx) = self.event_tx {
            let _ = tx.send(AgentEvent::ToolResultChunk {
                call_id: self.call_id.clone(),
                chunk,
            });
        }
    }

    /// Configure throttle interval (for tests or special cases)
    ///
    /// Default is 100ms. Lower values = more events (higher TUI load).
    pub fn set_throttle_interval(&self, interval: Duration) {
        let mut state = self.throttle_state.lock().unwrap();
        state.min_interval = interval;
    }
}
```

## ToolResultAccumulator

A helper struct that tools can use (optional) to accumulate chunks and apply
truncation.

```rust
pub struct ToolResultAccumulator {
    /// Accumulated chunks
    chunks: Vec<ToolResultChunk>,
    /// Sequence counter for chunks
    next_sequence: u64,
    /// Total bytes accumulated
    total_bytes: usize,
    /// Total lines accumulated (if content is text)
    total_lines: usize,
    /// Truncation configuration
    config: TruncationConfig,
}

pub struct TruncationConfig {
    /// Maximum lines before truncation (default: 1000)
    pub max_lines: usize,
    /// Head window size (default: 500)
    pub head_lines: usize,
    /// Tail window size (default: 500)
    pub tail_lines: usize,
    /// Maximum bytes (default: 1MB)
    pub max_bytes: usize,
}

impl Default for TruncationConfig {
    fn default() -> Self {
        Self {
            max_lines: 1000,
            head_lines: 500,
            tail_lines: 500,
            max_bytes: 1024 * 1024,  // 1MB
        }
    }
}

impl ToolResultAccumulator {
    pub fn new() -> Self {
        Self::with_config(TruncationConfig::default())
    }

    pub fn with_config(config: TruncationConfig) -> Self {
        Self {
            chunks: Vec::new(),
            next_sequence: 0,
            total_bytes: 0,
            total_lines: 0,
            config,
        }
    }

    /// Add a chunk (automatically assigns sequence number)
    pub fn push(&mut self, mut chunk: ToolResultChunk) {
        chunk.sequence = self.next_sequence;
        self.next_sequence += 1;

        self.total_bytes += chunk.content.len();
        if chunk.content_type == "text" {
            self.total_lines += chunk.content.lines().count();
        }

        self.chunks.push(chunk);
    }

    /// Create a chunk and add it
    pub fn push_text(&mut self, text: impl Into<String>) {
        self.push(ToolResultChunk::text(text));
    }

    /// Get total lines
    pub fn total_lines(&self) -> usize {
        self.total_lines
    }

    /// Get total bytes
    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Finalize: merge chunks, apply truncation, return ToolResult
    pub fn finalize(self) -> ToolResult {
        if self.chunks.is_empty() {
            return ToolResult::text("");
        }

        // Merge all text chunks
        let mut lines: Vec<String> = Vec::new();
        for chunk in &self.chunks {
            if chunk.content_type == "text" {
                for line in chunk.content.lines() {
                    lines.push(line.to_string());
                }
            }
        }

        // Apply truncation if needed
        let result_text = if lines.len() > self.config.max_lines {
            let head: Vec<_> = lines.iter()
                .take(self.config.head_lines)
                .map(|s| s.as_str())
                .collect();
            let tail: Vec<_> = lines.iter()
                .skip(lines.len() - self.config.tail_lines)
                .map(|s| s.as_str())
                .collect();

            let omitted = lines.len() - self.config.head_lines - self.config.tail_lines;
            let marker = format!("\n... [{} lines omitted] ...\n", omitted);

            let mut result = head.join("\n");
            result.push_str(&marker);
            result.push_str(&tail.join("\n"));
            result
        } else {
            lines.join("\n")
        };

        // Check if truncated
        let is_truncated = lines.len() > self.config.max_lines
            || self.total_bytes > self.config.max_bytes;

        ToolResult::text(result_text)
            .with_details(serde_json::json!({
                "total_lines": self.total_lines,
                "total_bytes": self.total_bytes,
                "truncated": is_truncated,
                "chunks": self.chunks.len(),
            }))
    }
}
```

## Usage Pattern

### Simple streaming (no accumulator)

Tool emits progress and result chunks directly, returns final summary:

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    let mut count = 0;

    for item in items {
        // Progress
        ctx.emit_structured_progress(
            ToolProgress::items(count, Some(total))
        );

        // Result chunk
        ctx.emit_result_chunk(
            ToolResultChunk::text(format!("Found: {}", item))
        );

        count += 1;
    }

    Ok(ToolResult::text(format!("Processed {} items", count)))
}
```

### With accumulator (for truncation)

Tool uses accumulator to collect chunks and apply truncation:

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    let mut accumulator = ToolResultAccumulator::new();
    let mut count = 0;

    for item in items {
        count += 1;

        // Progress
        ctx.emit_structured_progress(
            ToolProgress::items(count, Some(total))
        );

        // Chunk (both to TUI and accumulator)
        let chunk = ToolResultChunk::text(format!("Found: {}", item));
        ctx.emit_result_chunk(chunk.clone());
        accumulator.push(chunk);
    }

    // Return truncated result
    Ok(accumulator.finalize())
}
```

### Advanced: manual truncation

Tool can check `accumulator.total_lines()` mid-stream and stop early:

```rust
async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
    let mut accumulator = ToolResultAccumulator::with_config(
        TruncationConfig {
            max_lines: 10_000,
            head_lines: 5_000,
            tail_lines: 5_000,
            max_bytes: 10 * 1024 * 1024,  // 10MB
        }
    );

    for item in items {
        // Early termination if we've collected enough
        if accumulator.total_lines() >= 10_000 {
            ctx.emit_structured_progress(
                ToolProgress::phase("Truncating", 1, 1)
                    .with_message("Reached line limit")
            );
            break;
        }

        let chunk = ToolResultChunk::text(format!("{:?}", item));
        ctx.emit_result_chunk(chunk.clone());
        accumulator.push(chunk);
    }

    Ok(accumulator.finalize())
}
```

## Integration with Agent Loop

The agent executor needs minimal changes:

1. When a tool call starts, create a `ToolResultAccumulator` for that call_id
2. Listen for `AgentEvent::ToolResultChunk` events on the event bus
3. When a chunk arrives, add it to the accumulator
4. When tool completes, call `accumulator.finalize()` to get the final result
5. If the tool returns a result directly (not using accumulator), use that instead

```rust
// In agent executor
let accumulator = Arc::new(Mutex::new(ToolResultAccumulator::new()));

// Spawn event listener
let accumulator_clone = accumulator.clone();
tokio::spawn(async move {
    let mut rx = event_bus.subscribe();
    while let Ok(event) = rx.recv().await {
        if let AgentEvent::ToolResultChunk { call_id, chunk } = event {
            if call_id == tool_call.call_id {
                accumulator_clone.lock().unwrap().push(chunk);
            }
        }
    }
});

// Execute tool
let tool_result = tool.execute(&ctx, params).await?;

// If tool returned empty/summary result, use accumulator
let final_result = if tool_result.is_minimal() {
    accumulator.lock().unwrap().finalize()
} else {
    tool_result
};
```

## File Locations

- `src/tools/mod.rs` — extend `ToolContext` impl
- `src/tools/progress.rs` — new file for `ToolProgress`, `ProgressKind`, `ToolResultChunk`, `ToolResultAccumulator`
- `src/agent/executor.rs` — integrate accumulator into tool execution loop
