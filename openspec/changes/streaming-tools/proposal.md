# streaming-tools — Progressive Output for Long-Running Tools

## Intent

Long-running tools like `grep` on large codebases, `find` in deep directory
trees, `web_fetch` on slow endpoints, and `bash` commands that process
gigabytes of data currently batch their entire output and return it all at
once at the end. This creates multiple problems:

- **No visibility**: User and agent can't see progress during a 30-second grep
- **Memory bloat**: Collecting 50MB of grep matches in-process before truncation
- **Poor UX**: TUI shows "running..." for minutes with no feedback
- **No incremental feedback to LLM**: The model can't react to partial results mid-stream
- **Unbounded event bus flooding**: `emit_progress` is fire-and-forget with no back-pressure

The `bash` tool already streams stdout/stderr line-by-line via `emit_progress`,
and most tools call `emit_progress` for status updates. But `emit_progress` is
text-only, fire-and-forget, and the final `ToolResult` is still a single blob.

This change elevates streaming to a first-class protocol:
- Structured progress events (bytes processed, lines found, percentage complete)
- Progressive result streaming that the LLM can see incrementally
- Back-pressure and throttling to prevent event bus overload
- Smart truncation (head/tail windows for huge outputs)
- Cancellation UX (tools report cleanup progress)

## Scope

### In Scope

- New `ToolProgress` struct with structured progress metadata (ProgressKind enum)
- Extended `ToolContext` API: `emit_structured_progress` alongside existing `emit_progress`
- New `AgentEvent::ToolProgressUpdate` variant with structured progress
- Progressive result streaming: tools yield result chunks that accumulate
- Back-pressure strategy: ring buffer with drop-oldest policy (configurable)
- TUI progress rendering: progress bars, percentage, ETA, streaming output panel
- Smart truncation for huge outputs: head/tail window with "... N lines omitted ..."
- Cancellation reporting: tools emit "cancelling..." state before cleanup
- Migrate `bash`, `grep`, `find`, `web_fetch` tools to use streaming protocol
- Agent-visible partial results: streaming chunks appended to tool result as they arrive

### Out of Scope

- Changing the core `Tool` trait signature (keep `async fn execute` as-is)
- Replacing `emit_progress` entirely (it stays for simple text updates)
- Streaming result **to the LLM mid-generation** (tool still finishes before model sees final result)
  - _Future work: interrupt model generation with partial results, requires provider API changes_
- Progress estimation / ETA calculation in tool code (tools report facts, TUI calculates ETA)
- Per-tool rate limiting or quotas
- Network streaming (this is local agent → TUI only)
- Database or filesystem persistence of progress history

## Approach

Tools continue to implement the same `Tool` trait. The change is additive:

1. **ToolContext gains new methods**:
   - `emit_structured_progress(&self, progress: ToolProgress)` — structured events
   - `emit_result_chunk(&self, chunk: ToolResultChunk)` — progressive results
   - Existing `emit_progress(&self, text: &str)` remains unchanged

2. **New AgentEvent variants**:
   - `AgentEvent::ToolProgressUpdate { call_id, progress: ToolProgress }`
   - `AgentEvent::ToolResultChunk { call_id, chunk: ToolResultChunk }`
   - `AgentEvent::ToolExecutionUpdate` stays for backward compatibility

3. **Tools opt-in to streaming**:
   - Tools like `grep` call `emit_structured_progress` to report lines found
   - Tools call `emit_result_chunk` to yield results as they're produced
   - The event bus applies throttling (max 10 updates/sec per call_id)
   - Tools check `ctx.signal` for cancellation, emit "cancelling..." before cleanup

4. **TUI renders progress**:
   - New `ProgressRenderer` component shows progress bars, percentage, stats
   - Streaming output panel accumulates chunks with smart truncation
   - Head/tail window: first 100 lines + last 100 lines, "... N omitted ..."

5. **Agent sees accumulated results**:
   - As chunks arrive, they're appended to an in-memory `ToolResultAccumulator`
   - When tool finishes, accumulated result (with truncation applied) is
     returned to the agent
   - The LLM sees the final truncated result (just like today, but streamed to TUI)

No changes to model provider API or streaming inference flow. This is purely
about improving tool visibility and UX, with the foundation laid for future
incremental-result-to-LLM work.
