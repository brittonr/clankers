## Why

When the TUI exits, it leaves the alternate screen and the entire conversation disappears from the terminal. A 30-minute session with dozens of tool calls, code blocks, and assistant responses — gone. The user has to `clankers session show` or re-attach to see what happened. Every other terminal app with alternate screen has this problem, but for a coding agent the conversation is the primary artifact and losing it from scrollback is painful.

`rat-inline` (used by the new `--inline` mode) can render styled content into scrollback. After `restore_terminal()` leaves the alternate screen, we can walk the conversation blocks and write a styled summary to stdout so the conversation persists in terminal scrollback.

## What Changes

- Route each TUI exit path through a shared `finalize_terminal_and_scrollback(...)` helper that restores the terminal, then renders the conversation to scrollback using `rat-inline`
- The dump walks `App.conversation.blocks`, converting each block's prompt and responses into an `InlineView`
- Reuses `InlineRenderer`, `InlineMarkdown`, and `InlineText` from `rat-inline`
- Controlled by a setting (`scrollback_on_exit: Option<bool>`) so users can disable it explicitly, while `None` keeps the default enabled behavior

## Capabilities

### New Capabilities
- `tui-exit-scrollback`: Renders the conversation to terminal scrollback when the TUI exits. Covers block conversion, styling, truncation for long sessions, and the opt-out setting.

### Modified Capabilities

_(none)_

## Impact

- `src/modes/scrollback_dump.rs`: shared helpers that take `&[BlockEntry]` plus `&Settings`, restore the terminal, and write rendered scrollback to stdout via `InlineRenderer`
- `src/modes/interactive.rs`: call dump after `restore_terminal()`
- `src/modes/attach.rs`: call dump after `restore_terminal()`
- `src/modes/auto_daemon.rs`: call dump after `restore_terminal()`
- `crates/clankers-config/src/settings.rs`: add `scrollback_on_exit` setting
- Dependency: `rat-inline` already in workspace from the inline-rendering-mode change
