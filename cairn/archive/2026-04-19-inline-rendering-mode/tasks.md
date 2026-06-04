## 1. Dependencies and CLI

- [x] 1.1 Add `rat-inline` path dependency to root `Cargo.toml` (`rat-inline = { path = "../subwayrat/crates/rat-inline" }`)
- [x] 1.2 Add `Inline` variant to `OutputMode` enum in `src/cli.rs`
- [x] 1.3 Add `--inline` shorthand flag to `Cli` struct (sets mode to Inline when combined with `-p`)
- [x] 1.4 Verify `cargo check` passes with the new variant (handle exhaustive match)

## 2. Inline state model

- [x] 2.1 Create `src/modes/inline.rs` with `InlineState` struct — accumulates messages, tool calls, tool outputs, thinking blocks, and turn indices
- [x] 2.2 Implement `InlineState::apply(&mut self, event: AgentEvent)` — updates state from each event type (MessageUpdate, ToolCall, ToolExecutionUpdate, ToolExecutionEnd, ContentBlockStart, TurnStart/End, AgentEnd)
- [x] 2.3 Implement `InlineState::build_view(&self) -> InlineView` — constructs the full view tree with keyed nodes for each message, tool call, and tool result
- [x] 2.4 Add module to `src/modes/mod.rs`

## 3. View composition

- [x] 3.1 Render assistant text as `InlineMarkdown` with keyed nodes per message (`msg-{turn}-{block}`)
- [x] 3.2 Render tool call headers as styled `InlineText` (`⚡ {tool_name}: {summary}`) with key `tool-{call_id}`
- [x] 3.3 Render tool execution output as dimmed `InlineText` with key `tool-out-{call_id}`
- [x] 3.4 Render tool errors in red/bold style
- [x] 3.5 Render thinking blocks as dimmed italic `InlineText` with "Thinking..." prefix
- [x] 3.6 Render turn separators as thin horizontal rule `InlineText`
- [x] 3.7 Render usage stats footer when `show_stats` is true

## 4. Event loop and output

- [x] 4.1 Implement `run_inline_with_options()` — creates agent, subscribes to events, spawns render task
- [x] 4.2 Render task: loop over events, call `state.apply()`, `state.build_view()`, `renderer.rebuild()`, `renderer.render()`, write to stdout
- [x] 4.3 Detect terminal width via `crossterm::terminal::size()`, fall back to 80
- [x] 4.4 Reset terminal style (`\x1b[0m`) and print final newline on completion

## 5. Wire into main

- [x] 5.1 Add `OutputMode::Inline` match arm in `run_headless()` that calls `run_inline_with_options()`
- [x] 5.2 Pass through `PrintOptions`-equivalent config (output_file, show_stats, show_tools, thinking)
- [x] 5.3 Handle `--inline` flag: if set and `-p` is present, override mode to `Inline`

## 6. Testing

- [x] 6.1 Unit test `InlineState::apply` with a sequence of events, verify state accumulation
- [x] 6.2 Unit test `InlineState::build_view` produces correct node count and keys for a multi-turn conversation
- [x] 6.3 Integration test: construct an `InlineState`, feed representative events, build view, render to buffer, verify output is non-empty and contains expected text fragments
