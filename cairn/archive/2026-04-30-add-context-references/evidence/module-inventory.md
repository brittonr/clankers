# Context References Module Inventory

## Existing implementation

- `crates/clankers-util/src/at_file.rs` owns the current `@path` parser and expander. It already supports text files, line ranges, directory listings, and image attachment blocks.
- `src/modes/event_handlers.rs` expands `@` references in the interactive TUI before sending prompts to the agent.
- `src/modes/attach.rs` expands `@` references before daemon prompt submission, but the current attach client path sends only expanded text and drops image blocks.
- `crates/clankers-controller/src/client.rs` sends daemon prompts as `SessionCommand::Prompt { text, images: vec![] }`.
- `crates/clankers-protocol/src/command.rs` defines the daemon prompt command shape with text plus image blocks.
- `crates/clankers-controller/src/command.rs` routes daemon prompt commands to the controller prompt handler.
- `crates/clankers-session/src/entry.rs` already has `SessionEntry::Custom(CustomEntry)`, which is suitable for normalized context-reference metadata without adding a new entry variant.
- `src/tools/git_ops/diff.rs` contains git diff helpers that can back a future `@diff`/git-reference slice.

## Ownership decision

The first implementation slice should keep parsing/resolution in `clankers-util`, initially by extending or wrapping `at_file.rs` behind a generalized context-reference API. Prompt-mode callers should use that shared API rather than duplicating expansion logic.

Session/replay metadata should be recorded with `SessionEntry::Custom` when the prompt path has a session manager available. User-facing errors should be explicit in the expanded prompt text for the first slice, with structured metadata added as the session-observability task.

## First-pass supported cases

Supported now or in the immediate adapter slice:

- `@path/to/file`
- `@path/to/file:line` and `@path/to/file:start-end`
- `@path/to/dir/`
- image file references (`.jpg`, `.jpeg`, `.png`, `.gif`, `.webp`) as image content blocks where the caller supports images

Explicitly unsupported until later tasks or follow-up changes:

- URL references such as `@https://...`
- session artifact references
- rich git-diff references beyond the existing helper inventory
- remote fetches or credential-bearing references

## Prompt paths to wire

- Existing TUI expansion: `src/modes/event_handlers.rs`
- Existing attach expansion: `src/modes/attach.rs`
- Non-interactive prompt modes that currently call `agent.prompt(...)` directly: `src/modes/json.rs`, `src/modes/print.rs`, and `src/modes/inline.rs`
- Scheduled prompts: `src/modes/schedule_prompt.rs` should either use the same API or document that `@` expansion is not supported there yet.

## Verification evidence

This inventory was produced by delegated read-only inspection plus direct inspection of the active OpenSpec artifacts and current `at_file.rs` implementation. The repository was clean at commit `39aed557` before starting this task.
