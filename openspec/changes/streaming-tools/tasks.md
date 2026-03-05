# streaming-tools — Tasks

## Phase 1: Core Protocol (no TUI, no tool changes)

- [ ] Create `src/tools/progress.rs` with `ToolProgress`, `ProgressKind`, `ToolResultChunk` structs
- [ ] Implement `ProgressKind` enum with variants: `Bytes`, `Lines`, `Items`, `Percentage`, `Phase`
- [ ] Implement `ProgressKind::as_percentage()` method for uniform percentage calculation
- [ ] Implement `ProgressKind::display_string()` for human-readable formatting
- [ ] Implement `ToolProgress` builder methods: `bytes()`, `lines()`, `items()`, `percentage()`, `phase()`
- [ ] Implement `ToolResultChunk` builder methods: `text()`, `base64()`, `json()`
- [ ] Add `ToolProgressUpdate` and `ToolResultChunk` variants to `AgentEvent` in `src/agent/events.rs`
- [ ] Unit tests for `ProgressKind` percentage calculation and display strings
- [ ] Unit tests for `ToolProgress` and `ToolResultChunk` builders

## Phase 2: ToolContext Extensions (streaming API)

- [ ] Add `throttle_state: Arc<Mutex<ThrottleState>>` field to `ToolContext` in `src/tools/mod.rs`
- [ ] Create `ThrottleState` struct with `last_progress_emit: Option<Instant>` and `min_interval: Duration`
- [ ] Implement `ToolContext::emit_structured_progress(&self, progress: ToolProgress)` with throttling
- [ ] Implement `ToolContext::emit_result_chunk(&self, chunk: ToolResultChunk)` (no throttling)
- [ ] Implement `ToolContext::set_throttle_interval(&self, interval: Duration)` for configurability
- [ ] Update `ToolContext::new()` to initialize throttle state
- [ ] Unit tests: throttling behavior (rapid calls should be dropped)
- [ ] Unit tests: emit_structured_progress with no event channel (should not panic)
- [ ] Unit tests: emit_result_chunk ordering and sequence

## Phase 3: Result Accumulator

- [ ] Create `ToolResultAccumulator` struct in `src/tools/progress.rs`
- [ ] Create `TruncationConfig` struct with `max_lines`, `head_lines`, `tail_lines`, `max_bytes`
- [ ] Implement `ToolResultAccumulator::new()` and `with_config()`
- [ ] Implement `ToolResultAccumulator::push()` — auto-assign sequence numbers
- [ ] Implement `ToolResultAccumulator::push_text()` convenience method
- [ ] Implement `ToolResultAccumulator::total_lines()` and `total_bytes()` getters
- [ ] Implement `ToolResultAccumulator::finalize()` — merge chunks, apply head/tail truncation
- [ ] Add `... [N lines omitted] ...` marker insertion in `finalize()`
- [ ] Unit tests: accumulator with no truncation (small output)
- [ ] Unit tests: accumulator with head/tail truncation (large output)
- [ ] Unit tests: accumulator respects max_bytes limit
- [ ] Unit tests: chunk sequence numbering

## Phase 4: Agent Executor Integration

- [ ] Update `src/agent/executor.rs` to create `ToolResultAccumulator` per tool call
- [ ] Spawn event listener task to feed `ToolResultChunk` events into accumulator
- [ ] Wire `accumulator.finalize()` into tool completion flow
- [ ] Handle case where tool returns direct result vs accumulated chunks (prefer direct)
- [ ] Add `details` field to final `ToolResult` with `total_lines`, `total_bytes`, `truncated`, `chunks` metadata
- [ ] Integration test: tool emits chunks, executor accumulates, agent receives truncated result
- [ ] Integration test: tool emits no chunks, executor uses direct result

## Phase 5: Migrate Bash Tool to Streaming

- [ ] Update `src/tools/bash.rs` to emit `ToolProgress::lines()` for line count
- [ ] Update bash to emit `ToolResultChunk::text()` for stdout/stderr lines
- [ ] Keep existing `emit_progress()` calls for backward compatibility
- [ ] Add cancellation reporting: emit `ToolProgress::phase("Cancelling")` on signal
- [ ] Integration test: bash tool streams output, shows progress, handles cancellation

## Phase 6: Migrate Grep Tool to Streaming

- [ ] Update `src/tools/grep.rs` (or equivalent) to emit `ToolProgress::lines()` for matches found
- [ ] Emit `ToolResultChunk::text()` for each match line
- [ ] Add message to progress: "Searching <path>"
- [ ] Use `ToolResultAccumulator` to apply head/tail truncation for huge results
- [ ] Integration test: grep on large codebase, verify truncation, verify progress updates

## Phase 7: Migrate Find Tool to Streaming

- [ ] Update `src/tools/find.rs` (or equivalent) to emit `ToolProgress::items()` for files found
- [ ] Emit `ToolResultChunk::text()` for each found path
- [ ] Add message to progress: "Scanning <directory>"
- [ ] Handle cancellation gracefully (emit phase, stop traversal)
- [ ] Integration test: find in deep directory tree, verify streaming, verify cancellation

## Phase 8: Migrate Web Fetch Tool to Streaming

- [ ] Update `src/tools/web.rs` to emit `ToolProgress::bytes()` for download progress
- [ ] Parse `Content-Length` header to set total bytes
- [ ] Emit `ToolResultChunk::base64()` for binary content chunks
- [ ] Emit `ToolResultChunk::text()` for text/html content chunks
- [ ] Add phase progress: "Connecting", "Downloading", "Complete"
- [ ] Integration test: web_fetch downloads large file, shows progress bar with ETA

## Phase 9: TUI Progress Renderer

- [ ] Create `src/tui/components/progress_renderer.rs`
- [ ] Implement `ProgressRenderer` struct with `states: HashMap<String, ProgressState>`
- [ ] Implement `ProgressState` with `progress`, `started_at`, `updated_at`, `history`
- [ ] Implement `ProgressRenderer::update()` — store progress and history samples
- [ ] Implement `ProgressRenderer::render()` — delegate to kind-specific renderers
- [ ] Implement `render_countable()` — progress bar for bytes/lines/items with known total
- [ ] Implement `render_countable()` — spinner + count for unknown total
- [ ] Implement `render_percentage()` — progress bar for percentage kind
- [ ] Implement `render_phase()` — phase name + optional progress bar
- [ ] Implement `calculate_eta()` — linear regression over progress history
- [ ] Implement `spinner_char()` — animated spinner frames
- [ ] Add `format_duration()` helper for ETA display (e.g., "2m 30s")
- [ ] Unit tests: ETA calculation with mock history
- [ ] Unit tests: spinner animation cycles correctly

## Phase 10: TUI Streaming Output Panel

- [ ] Create `src/tui/components/streaming_output_panel.rs`
- [ ] Implement `StreamingOutputPanel` struct with `chunks: HashMap<String, ChunkBuffer>`
- [ ] Implement `ChunkBuffer` with smart head/tail truncation during accumulation
- [ ] Implement `StreamingOutputPanel::add_chunk()` — append chunk, apply truncation
- [ ] Implement `StreamingOutputPanel::render()` — display lines with scrolling
- [ ] Implement scrollbar rendering for tall content
- [ ] Implement `scroll_down()`, `scroll_up()`, `scroll_to_bottom()` methods
- [ ] Highlight omission marker lines with dimmed yellow style
- [ ] Show stats footer: lines/total/chunks count
- [ ] Unit tests: chunk accumulation with truncation
- [ ] Unit tests: scroll offset clamping

## Phase 11: TUI Integration

- [ ] Update `src/tui/components/tool_panel.rs` to embed `ProgressRenderer` and `StreamingOutputPanel`
- [ ] Add event handler for `AgentEvent::ToolProgressUpdate` — call `progress_renderer.update()`
- [ ] Add event handler for `AgentEvent::ToolResultChunk` — call `output_panel.add_chunk()`
- [ ] Keep existing `AgentEvent::ToolExecutionUpdate` handler for backward compatibility
- [ ] Layout tool panel: progress section (top 3 lines) + output section (remaining)
- [ ] Add keybindings: `j`/`k` scroll, `g`/`G` top/bottom, `f` toggle auto-follow
- [ ] Implement auto-follow mode (scroll to bottom on new chunks)
- [ ] Add detail view mode (show progress history, chunk metadata) on `d` key
- [ ] Update status bar to show active tool progress (e.g., "grep: 1234 lines")
- [ ] Integration test: TUI renders progress bars, streaming output, responds to keys

## Phase 12: Documentation and Examples

- [ ] Add doc comments to all new public types and methods
- [ ] Update `docs/tools.md` (if exists) with streaming API usage guide
- [ ] Create example tool in `examples/streaming_tool.rs` demonstrating all progress kinds
- [ ] Add streaming section to main README if not auto-generated
- [ ] Update CHANGELOG with "streaming-tools" feature entry
- [ ] Migration guide for existing tools (grep, find, web) in `docs/migration-streaming.md`

## Phase 13: Performance and Edge Cases

- [ ] Benchmark: throttling overhead (should be <1μs per emit call)
- [ ] Benchmark: accumulator truncation on 100K line output
- [ ] Benchmark: TUI render performance with 100 active streaming tools
- [ ] Test: rapid progress updates (1M/sec) should throttle correctly
- [ ] Test: event bus ring buffer overflow (should drop oldest, not crash)
- [ ] Test: cancellation during progress emit (should not deadlock)
- [ ] Test: accumulator with mixed text/base64 chunks
- [ ] Test: zero-total progress (edge case: total=0 should not divide by zero)
- [ ] Test: negative/NaN percentage (should clamp to 0-100)
- [ ] Fix any panics, deadlocks, or memory leaks found during stress testing
