## Why

clankers has two output modes: full-screen interactive TUI and headless print (plain text / JSON / markdown to stdout). The TUI is great for interactive sessions but can't be used in pipes, CI, or terminals where you want scrollback. The print mode works in those contexts but it's unstyled — raw text with no formatting, no spinners, no visual structure.

`rat-inline` (just landed in subwayrat) provides inline scrollback rendering: styled markdown, frame diffing, terminal growth, reconciliation. Adding an `--inline` output mode to clankers gives users rich styled output that accumulates in scrollback — styled markdown, tool call headers, streaming output, thinking indicators — without taking over the terminal.

## What Changes

- New `OutputMode::Inline` variant in the CLI enum
- New `clankers::modes::inline` module that consumes `AgentEvent`s and renders them via `rat-inline`'s `InlineRenderer`
- Wired into `run_headless` alongside the existing print/json/markdown paths
- `--mode inline` flag, or `--inline` shorthand

## Capabilities

### New Capabilities
- `inline-output-mode`: Renders agent events (streaming text, tool calls, tool output, thinking blocks) as styled inline terminal output using rat-inline. Covers event dispatch, view composition, and lifecycle.

### Modified Capabilities

_(none — existing modes are untouched)_

## Impact

- `src/cli.rs`: new `Inline` variant in `OutputMode`, `--inline` flag
- `src/main.rs`: new match arm in `run_headless` dispatching to inline mode
- New module: `src/modes/inline.rs`
- New dependency: `rat-inline` (from subwayrat workspace, path dep)
- `Cargo.toml`: add `rat-inline` dep
- Existing print/json/markdown modes: no changes
- Interactive TUI: no changes
