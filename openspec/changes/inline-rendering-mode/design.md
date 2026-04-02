## Context

clankers headless mode runs in `run_headless()` in `main.rs`. It builds an agent, subscribes to `AgentEvent`s via `agent.subscribe()`, and dispatches to one of three renderers based on `OutputMode`:

- `OutputMode::Json` → `modes::json::run_json_with_options()`
- `OutputMode::Markdown` → `modes::print::run_print_with_options()` (markdown format)
- `_ (Print/Plain)` → `modes::print::run_print_with_options()` (text format)

Each renderer spawns a tokio task that reads from the event channel and writes to stdout. The inline mode follows the same pattern — it's another renderer, not a new agent architecture.

`rat-inline` (in `../subwayrat/crates/rat-inline`) provides `InlineRenderer`, `InlineView` builder, `InlineMarkdown`, and `InlineText`. The reconciler preserves state across rebuilds. Frame diffing minimizes ANSI output.

## Goals / Non-Goals

**Goals:**
- `clankers -p "fix the tests" --inline` renders styled output to scrollback
- Streaming markdown with syntax highlighting for assistant messages
- Tool call/result headers with visual structure
- Thinking block indicators
- Token usage stats at the end
- Works in pipes (degrades gracefully when not a terminal)

**Non-Goals:**
- Input handling — inline mode is one-shot like print mode, not interactive
- Spinners/animation — needs a tick mechanism, follow-up
- Replacing the TUI — inline is for headless one-shot, TUI is for interactive
- Session persistence — inline mode uses the same session infra as print mode

## Decisions

### 1. Same architecture as print mode

The inline renderer follows the exact pattern of `modes::print`: receive `AgentEvent`s from a channel, accumulate state, write to stdout. The only difference is the output is styled via `rat-inline` instead of raw text.

```rust
// Spawn event consumer (same pattern as print.rs)
let handle = tokio::spawn(async move {
    let mut renderer = InlineRenderer::new(width);
    let mut state = InlineState::new();
    while let Ok(event) = rx.recv().await {
        state.apply(event);
        let view = state.build_view();
        renderer.rebuild(view);
        let output = renderer.render();
        stdout.write_all(&output)?;
        stdout.flush()?;
    }
});
```

**Alternative considered**: Running the inline renderer as a separate thread with a tick loop. Rejected — there's no animation yet, so event-driven rendering is sufficient and simpler.

### 2. State accumulation, not event-by-event rendering

The inline renderer maintains an `InlineState` that accumulates all messages, tool calls, and results. On each event, it rebuilds the full view tree and lets the reconciler + diff engine figure out what changed. This is the same model as eye-declare and React — declarative full-tree rebuilds with efficient diffing.

This is cleaner than trying to incrementally mutate the view tree on each event.

### 3. Markdown for assistant text, plain for tool output

Assistant message content renders through `InlineMarkdown` (styled headings, bold, code blocks). Tool call inputs and results render as `InlineText` with dimmed styling — same as how the print mode shows them but with color. Tool streaming output (stdout/stderr from bash, etc.) renders as `InlineText` with monospace gray styling.

### 4. Width detection with fallback

Use `crossterm::terminal::size()` for width. If it fails (piped, no terminal), fall back to 80 columns. The renderer still produces styled output (ANSI escapes) — some terminals and pagers (less -R) can display it.

### 5. `--inline` shorthand

Add `--inline` as a CLI flag that sets `mode = OutputMode::Inline`. Equivalent to `--mode inline`. Mirrors the existing `-p` shorthand for print mode.

## Risks / Trade-offs

- **[No animation]** Without a tick loop, there are no spinners or progress animations. The user sees text appear as events arrive, but nothing moves between events. → Acceptable for v1, add spinners in follow-up.
- **[Pipe behavior]** When piped, ANSI escapes go into the pipe. → Mitigation: detect `!isatty(stdout)` and fall back to plain print mode, or let the user force it with `--mode inline`.
- **[Streaming delta granularity]** `MessageUpdate` events can fire per-token. Rebuilding the full view tree per token could be expensive with many messages. → Mitigation: The reconciler is O(N) and the diff engine only emits changed cells. With typical conversation lengths (<100 nodes), this is microseconds.
