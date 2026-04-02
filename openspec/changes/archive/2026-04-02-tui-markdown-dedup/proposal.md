## Why

`clankers-tui/src/components/markdown.rs` (730 lines) is a stale fork of `rat-markdown` in subwayrat. They share the same parser, same rendering logic, same tests — the diff is ~50 lines of naming and a `from_theme()` convenience method. When bugs get fixed or features added to one copy, the other drifts. The `SyntaxHighlighter` trait is also duplicated: `clankers-tui-types` defines its own, and `rat-markdown` defines its own, so calling code can't mix them.

## What Changes

- Replace `clankers-tui/src/components/markdown.rs` with a `rat-markdown` dependency
- Unify the `SyntaxHighlighter` trait: clankers-tui-types re-exports from `rat-markdown` instead of defining its own
- Move the `MarkdownStyle::from_theme()` constructor to the TUI crate (thin wrapper over `rat-markdown::MarkdownStyle::from_base()`)
- Remove the duplicated 730-line file

## Capabilities

### New Capabilities

_(none — this is a deduplication, not a new feature)_

### Modified Capabilities

_(none — rendering behavior is identical)_

## Impact

- `crates/clankers-tui/Cargo.toml`: add `rat-markdown` dependency
- `crates/clankers-tui/src/components/markdown.rs`: delete (replaced by re-export + thin wrapper)
- `crates/clankers-tui/src/components/mod.rs`: update module declaration
- `crates/clankers-tui-types/Cargo.toml`: add `rat-markdown` dependency
- `crates/clankers-tui-types/src/lib.rs`: re-export `SyntaxHighlighter` and `PlainHighlighter` from `rat-markdown` instead of defining locally
- All call sites that import `crate::components::markdown::*` need path updates
- `render_markdown` call sites unchanged (same function signature)
