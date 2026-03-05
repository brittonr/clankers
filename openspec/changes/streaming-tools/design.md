# streaming-tools — Design

## Decisions

### Extend ToolContext, don't create StreamingToolContext

**Choice:** Add `emit_structured_progress` and `emit_result_chunk` methods to
the existing `ToolContext`.
**Rationale:** Tools already have a `ToolContext` parameter. Adding methods is
non-breaking — tools can opt-in when ready. A separate `StreamingToolContext`
would require changing every tool signature or maintaining two parallel trait
hierarchies. The existing `emit_progress` stays for simple cases.
**Alternatives considered:** New `StreamingTool` trait with `execute_streaming`,
separate `StreamingToolContext`. Both add complexity and split the tool
ecosystem. Extending the existing context is the Rust-idiomatic choice (see
`tokio::io::AsyncRead` → `AsyncReadExt`).

### ProgressKind enum for structured progress metadata

**Choice:** Define `ProgressKind` enum with variants: `Bytes`, `Lines`, `Items`,
`Percentage`, `Phase`.
```rust
pub enum ProgressKind {
    Bytes { current: u64, total: Option<u64> },
    Lines { current: u64, total: Option<u64> },
    Items { current: u64, total: Option<u64> },
    Percentage { percent: f32 },  // 0.0 to 100.0
    Phase { name: String, step: u32, total_steps: Option<u32> },
}
```
**Rationale:** Tools produce different kinds of progress. Grep counts lines,
web_fetch counts bytes, nix builds go through phases. A structured enum lets
the TUI render appropriate widgets (progress bar vs spinner vs phase name).
Text-only progress forces the TUI to parse strings.
**Alternatives considered:** Single `{ current: u64, total: Option<u64>, unit: String }`
struct. Simpler but loses type safety and semantic meaning. The TUI can't
reliably distinguish bytes from lines without parsing unit strings.

### Ring buffer with drop-oldest back-pressure

**Choice:** Event bus maintains a per-call_id ring buffer (size: 1000 events).
When full, oldest events are dropped. Tools never block on `emit_progress`.
**Rationale:** Tools should never block waiting for the TUI to catch up.
Dropping old progress updates is fine — the latest state matters most. If the
TUI is lagging, it'll skip intermediate states and jump to the current one.
Blocking would cause tools to hang if the TUI panel isn't even open.
**Alternatives considered:**
- Block when buffer is full: Deadlock risk if TUI stops consuming.
- Unbounded channel: OOM risk on huge grep output.
- Drop newest: User sees stale state.
Ring buffer + drop-oldest is the standard solution (see terminal scrollback).

### Throttle structured progress to 10 updates/sec per tool

**Choice:** `ToolContext::emit_structured_progress` checks if ≥100ms has elapsed
since the last emission for this call_id. If not, the event is dropped silently.
**Rationale:** A grep that finds 100K matches/sec would flood the event bus and
peg the TUI render loop. 10 updates/sec is smooth enough for human perception
(film is 24fps) and keeps event traffic manageable. Tools can emit as often as
they want — the throttle is transparent.
**Alternatives considered:**
- No throttling: Event bus overload, TUI render stutter.
- Throttle at event bus: Centralized but requires per-call_id state in the bus.
- Sample-based (emit every Nth): Tools need to track counters, more complex.
Time-based throttling at the context layer is simplest and most predictable.

### Progressive result chunks append to in-memory accumulator

**Choice:** `ToolResultAccumulator` struct holds `Vec<ToolResultChunk>`, appends
as chunks arrive, applies truncation at the end.
```rust
struct ToolResultAccumulator {
    chunks: Vec<ToolResultChunk>,
    total_bytes: usize,
    total_lines: usize,
    truncation_config: TruncationConfig,
}
```
**Rationale:** The agent still receives one final `ToolResult` when the tool
finishes (existing behavior preserved). Chunks stream to the TUI for live
updates. The accumulator merges chunks and truncates if needed (e.g., head/tail
window for huge grep output). This avoids changing tool return types or the
agent loop.
**Alternatives considered:**
- Tools return `Stream<ToolResultChunk>`: Requires async streams in the Tool
  trait, major breaking change.
- No accumulator, TUI only: Agent loses visibility into chunked data.
Current approach is additive and backward-compatible.

### Head/tail truncation with omission marker

**Choice:** When `total_lines > max_lines` (default 1000), keep first 500 lines
+ last 500 lines. Insert a marker line: `... [N lines omitted] ...` in between.
**Rationale:** For grep on a huge codebase, you want to see the first matches
(context) and the last matches (end of search). Middle results are often
repetitive. The marker line makes truncation obvious. This is how `git diff`
and `less` work.
**Alternatives considered:**
- Head-only: Lose tail context.
- Tail-only: Lose head context.
- Sampling (every Nth line): Loses semantic structure (e.g., function boundaries).
Head/tail preserves both ends and is familiar UX.

### Cancellation reporting as a ProgressKind::Phase

**Choice:** When a tool detects `ctx.signal.is_cancelled()`, it emits:
```rust
ctx.emit_structured_progress(ToolProgress {
    kind: ProgressKind::Phase {
        name: "Cancelling...".to_string(),
        step: 1,
        total_steps: Some(1),
    },
    message: Some("Cleaning up partial results".to_string()),
});
```
Then performs cleanup (delete temp files, kill child processes) before returning.
**Rationale:** Users need feedback that cancellation was received. A long-running
grep might take seconds to cancel if it's deep in a syscall. The "Cancelling..."
phase gives immediate feedback. Tools can emit multiple phases if cleanup is
multi-step (e.g., "Killing child process", "Removing temp files").
**Alternatives considered:**
- Silent cancellation: User doesn't know if Ctrl+C was ignored.
- New `AgentEvent::ToolCancelling`: Extra event type for rare case.
Using `Phase` reuses existing infrastructure and is semantically correct.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                          Tool                               │
│                                                             │
│  async fn execute(ctx: &ToolContext, ...) -> ToolResult {  │
│                                                             │
│    for item in iterator {                                  │
│      // Structured progress                                │
│      ctx.emit_structured_progress(ToolProgress {           │
│        kind: ProgressKind::Lines { current: n, total },    │
│        message: Some("Searching...".to_string()),          │
│      });                                                   │
│                                                             │
│      // Result chunk                                       │
│      ctx.emit_result_chunk(ToolResultChunk::text(line));   │
│                                                             │
│      // Check cancellation                                 │
│      if ctx.signal.is_cancelled() {                        │
│        ctx.emit_structured_progress(                       │
│          ToolProgress::phase("Cancelling...", 1, 1)        │
│        );                                                  │
│        cleanup();                                          │
│        return ToolResult::cancelled();                     │
│      }                                                     │
│    }                                                       │
│                                                             │
│    Ok(ToolResult::success(accumulated))                    │
│  }                                                         │
│                                                             │
└──────────────┬──────────────────────────────────────────────┘
               │
               │ emit_structured_progress / emit_result_chunk
               ▼
┌─────────────────────────────────────────────────────────────┐
│                      ToolContext                            │
│                                                             │
│  emit_progress(&self, text: &str)  [existing, unchanged]   │
│  emit_structured_progress(&self, progress: ToolProgress)   │
│  emit_result_chunk(&self, chunk: ToolResultChunk)          │
│                                                             │
│  Throttling: track last_emit_time per call_id             │
│  If elapsed < 100ms, drop silently                         │
│                                                             │
└──────────────┬──────────────────────────────────────────────┘
               │
               │ Send to event_tx (broadcast channel)
               ▼
┌─────────────────────────────────────────────────────────────┐
│                 Event Bus (broadcast channel)               │
│                                                             │
│  Per-subscriber ring buffer (1000 events)                  │
│  Drop oldest on overflow                                   │
│                                                             │
└───────┬──────────────────────────┬──────────────────────────┘
        │                          │
        │                          │
        ▼                          ▼
┌──────────────────┐    ┌─────────────────────────────────────┐
│  TUI Renderer    │    │  ToolResultAccumulator             │
│                  │    │                                     │
│  Progress Panel  │    │  chunks: Vec<ToolResultChunk>      │
│  - Progress bar  │    │  total_bytes: usize                │
│  - Percentage    │    │  total_lines: usize                │
│  - ETA estimate  │    │                                     │
│  - Phase name    │    │  accumulate() -> merge chunks      │
│                  │    │  truncate() -> head/tail window    │
│  Output Panel    │    │  finalize() -> ToolResult          │
│  - Streaming     │    │                                     │
│  - Head/tail     │    └─────────────────────────────────────┘
│  - Truncation    │                  │
│                  │                  │
└──────────────────┘                  │
                                      ▼
                              ┌───────────────────┐
                              │  Agent Loop       │
                              │                   │
                              │  Receives final   │
                              │  ToolResult       │
                              └───────────────────┘
```

## Data Flow

### Progress update
1. Tool calls `ctx.emit_structured_progress(ToolProgress { kind: ProgressKind::Lines { current: 500, total: Some(1000) }, ... })`
2. ToolContext checks throttle: if <100ms since last emit for this call_id, drop
3. Emit `AgentEvent::ToolProgressUpdate { call_id, progress }` to event bus
4. Event bus adds to ring buffer, drops oldest if full
5. TUI progress panel receives event, renders progress bar at 50%

### Result chunk
1. Tool calls `ctx.emit_result_chunk(ToolResultChunk::text("found match: ..."))`
2. ToolContext emits `AgentEvent::ToolResultChunk { call_id, chunk }` (no throttle for results)
3. TUI output panel appends chunk to scrollback
4. ToolResultAccumulator appends chunk to in-memory buffer
5. If total_lines > max_lines, accumulator marks for truncation (applied at end)

### Cancellation
1. User presses Ctrl+C, `ctx.signal` is cancelled
2. Tool detects `ctx.signal.is_cancelled()` in loop
3. Tool emits `ToolProgress::phase("Cancelling...", 1, 1)`
4. Tool performs cleanup (kill children, remove temp files)
5. Tool returns `ToolResult::cancelled()`
6. TUI shows "Cancelled" badge, final partial output

### Final result
1. Tool completes, returns `ToolResult`
2. Agent executor calls `accumulator.finalize()` to get merged, truncated result
3. If result was streamed, use accumulator's result; otherwise use tool's direct return
4. Agent sends result to LLM in next turn
5. TUI shows final status (success/error/cancelled)
