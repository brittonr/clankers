## Context

The TUI runs on the alternate screen. When it exits (`/quit`, Ctrl-C, agent finishes), `restore_terminal()` disables raw mode, leaves the alternate screen, and shows the cursor. At that point the terminal shows whatever was in the main screen buffer before the TUI launched — the conversation is gone.

Three exit paths need the same terminal-finalization behavior:
- `src/modes/interactive.rs` — standalone interactive mode
- `src/modes/attach.rs` — attached to daemon session
- `src/modes/auto_daemon.rs` — auto-daemon attach

All three have access to the `App` conversation entries at the point of exit, so they can route through one shared finalizer.

## Goals / Non-Goals

**Goals:**
- After TUI exit, the conversation appears in terminal scrollback with styled markdown, tool headers, and basic structure
- Works for all three exit paths
- Opt-out via setting for users who don't want scrollback noise
- Truncation for very long sessions (don't dump 500 blocks)

**Non-Goals:**
- Full fidelity reproduction of the TUI layout (borders, panels, split views)
- Images or file previews
- Interactive scrollback (it's write-once)

## Decisions

### 1. Shared dump function in a new module

Create `src/modes/scrollback_dump.rs` with shared helpers:
```rust
pub fn finalize_terminal_and_scrollback(
    run_result: Result<()>,
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    entries: &[BlockEntry],
    settings: &Settings,
) -> Result<()>

pub fn dump_conversation_to_scrollback(entries: &[BlockEntry], settings: &Settings) -> Result<()>
```

The dump helper walks `BlockEntry` values, keeps only conversation blocks, builds an `InlineView` with keyed nodes for each block's prompt and responses, then renders and writes to stdout. The shared finalizer restores the terminal first, then invokes the dump helper so all three exit paths share one shell.

### 2. Block → InlineView conversion

Each `ConversationBlock` maps to:
- A separator line (block index, timestamp)
- The user prompt as bold text
- Each `DisplayMessage` response:
  - `Assistant` → `InlineMarkdown`
  - `ToolCall` → styled header (`⚡ {name}`)
  - `ToolResult` → dimmed text (truncated to ~10 lines per tool)
  - `Thinking` → dimmed italic (first line only, or omitted)
- A blank line between blocks

This mirrors what the inline output mode does for live events, but reads from stored blocks instead.

### 3. Truncation

For sessions with more than 20 blocks, show the last 20 with a "... N earlier blocks omitted" header. The most recent context is what the user cares about when they just exited.

### 4. Width detection

Use `crossterm::terminal::size()` after `restore_terminal()` — at that point the terminal is back in normal mode and size detection works. Fall back to 80.

### 5. Setting

`scrollback_on_exit: Option<bool>` in settings. `Some(false)` disables the dump, while `None` or `Some(true)` keep the default enabled behavior. The setting is checked inside the dump helper together with the stdout-is-terminal guard.

## Risks / Trade-offs

- **[Terminal spam]** Long sessions produce a lot of scrollback. Mitigated by the 20-block truncation and the opt-out setting.
- **[Rendering time]** For 20 blocks with markdown, the dump takes a few milliseconds. Not noticeable.
- **[Piped terminals]** If stdout is piped (not a terminal), skip the dump entirely — there's no scrollback to write to.
