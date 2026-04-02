## 1. Settings

- [ ] 1.1 Add `scrollback_on_exit: Option<bool>` to `Settings` in `crates/clankers-config/src/settings.rs` (default `None`, treated as `true`)
- [ ] 1.2 Add serde handling so `"scrollback_on_exit": false` in settings.json disables the feature

## 2. Scrollback dump function

- [ ] 2.1 Create `src/modes/scrollback_dump.rs` with `pub fn dump_conversation_to_scrollback(blocks: &[ConversationBlock], settings: &Settings)`
- [ ] 2.2 Early return if `settings.scrollback_on_exit == Some(false)`
- [ ] 2.3 Early return if stdout is not a terminal (`!std::io::stdout().is_terminal()`)
- [ ] 2.4 Detect terminal width via `crossterm::terminal::size()`, fall back to 80
- [ ] 2.5 Truncation: if more than 20 blocks, render a "... N earlier blocks omitted" header and only the last 20
- [ ] 2.6 For each block, build an `InlineView` with:
  - Separator line with timestamp (`──── HH:MM:SS ────`)
  - User prompt as bold `InlineText`
  - Each `DisplayMessage` response mapped by role:
    - `Assistant` → `InlineMarkdown`
    - `ToolCall` → bold `InlineText` (`⚡ {name}`)
    - `ToolResult` → dimmed `InlineText` (truncated to 10 lines)
    - `Thinking` → dimmed italic (first line + "...")
  - Blank line between blocks
- [ ] 2.7 Create `InlineRenderer`, rebuild with the view, render, write to stdout, flush
- [ ] 2.8 Reset terminal style (`\x1b[0m`) after writing
- [ ] 2.9 Add module to `src/modes/mod.rs`

## 3. Wire into exit paths

- [ ] 3.1 In `src/modes/interactive.rs`, call `dump_conversation_to_scrollback()` after `restore_terminal()`, passing `app.conversation.blocks` and settings
- [ ] 3.2 In `src/modes/attach.rs`, call `dump_conversation_to_scrollback()` after `restore_terminal()`
- [ ] 3.3 In `src/modes/auto_daemon.rs`, call `dump_conversation_to_scrollback()` after `restore_terminal()`

## 4. Testing

- [ ] 4.1 Unit test: build an `InlineView` from a mock `ConversationBlock` with prompt + assistant + tool messages, verify non-empty render output
- [ ] 4.2 Unit test: truncation — 25 blocks produces view with omission header + 20 blocks
- [ ] 4.3 Unit test: empty conversation produces no output
