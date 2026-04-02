## 1. Dependency wiring

- [ ] 1.1 Add `rat-markdown` to `[workspace.dependencies]` in root `Cargo.toml` (`rat-markdown = { path = "../subwayrat/crates/rat-markdown" }`)
- [ ] 1.2 Add `rat-markdown = { workspace = true }` to `crates/clankers-tui-types/Cargo.toml`
- [ ] 1.3 Add `rat-markdown = { workspace = true }` to `crates/clankers-tui/Cargo.toml`

## 2. Unify the SyntaxHighlighter trait

- [ ] 2.1 In `crates/clankers-tui-types/src/syntax.rs`, delete the local `SyntaxHighlighter` trait, `HighlightSpan` struct, and `PlainHighlighter` — replace with re-exports from `rat_markdown` (`pub use rat_markdown::{SyntaxHighlighter, PlainHighlighter, HighlightSpan};`)
- [ ] 2.2 Verify `clankers-tui-types` re-exports are visible (check `pub use` in `lib.rs`)
- [ ] 2.3 Update `crates/clankers-tui/src/app/mod.rs` — `Box<dyn clankers_tui_types::SyntaxHighlighter>` should still work since it re-exports the rat-markdown trait

## 3. Replace markdown.rs with rat-markdown

- [ ] 3.1 In `crates/clankers-tui/src/components/markdown.rs`, delete the 700+ line parser/renderer and replace with: re-export of `rat_markdown::{MarkdownStyle, render_markdown}` plus a `markdown_style_from_theme(theme: &Theme) -> MarkdownStyle` constructor
- [ ] 3.2 Update imports in `crates/clankers-tui/src/components/block_view/render.rs` — `MarkdownStyle` and `render_markdown` now come from the re-export module
- [ ] 3.3 Update `highlighter` parameter types in `block_view/render.rs` and `block_view/mod.rs` — should still be `&dyn clankers_tui_types::SyntaxHighlighter` (which is now `rat_markdown::SyntaxHighlighter`)

## 4. Verify

- [ ] 4.1 `cargo check -p clankers-tui-types` passes
- [ ] 4.2 `cargo check -p clankers-tui` passes
- [ ] 4.3 `cargo check` (whole workspace) passes
- [ ] 4.4 `cargo nextest run -p clankers-tui` — markdown tests still pass (they moved to rat-markdown but the re-export should still expose the function for any remaining TUI-side tests)
