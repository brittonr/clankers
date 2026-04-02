## ADDED Requirements

### Requirement: Markdown rendering uses rat-markdown
The TUI SHALL use `rat-markdown` as the single source for markdown-to-styled-lines rendering. The `render_markdown` function, `MarkdownStyle` type, and `SyntaxHighlighter` trait SHALL come from the `rat-markdown` crate.

#### Scenario: Identical rendering output
- **WHEN** the TUI renders an assistant message containing markdown
- **THEN** the styled output SHALL be identical to the current rendering (same spans, same styles, same line breaks)

#### Scenario: Syntax highlighting compatibility
- **WHEN** a code block with a language tag is rendered
- **THEN** the TUI's syntect-based highlighter SHALL implement `rat_markdown::SyntaxHighlighter` and produce the same highlighted output as before

### Requirement: SyntaxHighlighter trait defined once
The `SyntaxHighlighter` trait SHALL be defined in `rat-markdown` and re-exported by `clankers-tui-types`. There SHALL NOT be duplicate trait definitions.

#### Scenario: Single trait identity
- **WHEN** code implements `SyntaxHighlighter`
- **THEN** it implements `rat_markdown::SyntaxHighlighter` (one trait, one impl)
