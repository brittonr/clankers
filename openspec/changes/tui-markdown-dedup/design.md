## Context

The TUI's `markdown.rs` was the original implementation. It was extracted into `rat-markdown` in subwayrat as part of the rat-inline work. The extraction added a standalone `SyntaxHighlighter` trait (rat-markdown can't depend on clankers-tui-types), renamed a few locals, and removed the `from_theme()` constructor that depends on clankers-tui's `Theme` type.

The two files are functionally identical — same parser state machine, same span construction, same test cases. The `render_markdown()` function signature differs only in which `SyntaxHighlighter` trait it takes.

## Goals / Non-Goals

**Goals:**
- Single source of truth for markdown rendering
- `SyntaxHighlighter` trait defined once, used everywhere
- Zero behavior change in TUI rendering

**Non-Goals:**
- Changing the markdown parser or adding features (follow-up)
- Modifying rat-markdown's API to accommodate clankers-specific needs

## Decisions

### 1. rat-markdown owns the trait, clankers re-exports

`rat-markdown` defines `SyntaxHighlighter`, `PlainHighlighter`, `HighlightSpan`, `MarkdownStyle`, and `render_markdown`. The `clankers-tui-types` crate re-exports the trait and `PlainHighlighter` so existing code that imports from `clankers_tui_types` doesn't break.

The TUI's syntect-based highlighter already implements the trait — it just needs to implement `rat_markdown::SyntaxHighlighter` instead of `clankers_tui_types::SyntaxHighlighter`. Since the trait signatures are identical, this is a path change.

### 2. `from_theme()` stays in clankers-tui as a free function

`MarkdownStyle::from_theme(theme)` depends on `Theme`, which is a clankers-tui type. Rather than pushing `Theme` into rat-markdown (wrong dependency direction), keep a `markdown_style_from_theme(theme: &Theme) -> MarkdownStyle` function in `clankers-tui/src/components/markdown.rs`. The file shrinks from 730 lines to ~20 (the constructor + re-exports).

### 3. Workspace dependency

`rat-markdown` is already transitively available (rat-inline depends on it). Add it to `[workspace.dependencies]` and to both `clankers-tui` and `clankers-tui-types` Cargo.toml files.

## Risks / Trade-offs

- **[Trait identity]** Rust treats `rat_markdown::SyntaxHighlighter` and `clankers_tui_types::SyntaxHighlighter` as different traits even if structurally identical. After the change, there's only one trait, so this is a fix not a risk. During the migration, all implementors and call sites must switch in the same commit.
- **[Upstream drift]** If rat-markdown diverges from what the TUI needs, we'd need to fork or extend. Mitigated by owning both repos.
