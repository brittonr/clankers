## 1. Settings and dump guardrails

- [x] I1 Add `scrollback_on_exit` config wiring in `crates/clankers-config/src/settings.rs` plus the shared dump entrypoint so explicit `false` skips rendering, unset or explicit `true` still render, and the dump short-circuits when stdout is not a terminal. [covers=tui.exit.scrollback.dump.on.exit.non-terminal-stdout-skips-dump,tui.exit.scrollback.dump.setting.opt-out,tui.exit.scrollback.dump.setting.opt-out.disabled-setting-skips-dump,tui.exit.scrollback.dump.setting.opt-out.default-or-true-enables-dump]
- [x] I2 Create `src/modes/scrollback_dump.rs` and its shared render pipeline so it detects terminal width with an 80-column fallback, converts each `ConversationBlock` into a timestamp separator, bold prompt, assistant markdown, thinking summaries rendered as dimmed italic first-line output or omitted consistently, and tool sections, truncates tool output after 10 lines with an omission indicator, and renders the final `InlineView` through `InlineRenderer` for scrollback output. [covers=tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation,tui.exit.scrollback.dump.content.structure,tui.exit.scrollback.dump.content.structure.block-rendering-preserves-prompt-markdown-and-tool-sections,tui.exit.scrollback.dump.content.structure.tool-output-truncates-after-ten-lines]
- [x] I3 Add 20-block truncation with a `... N earlier blocks omitted` header for long sessions and full rendering with no omission header for 20-or-fewer-block sessions. [covers=tui.exit.scrollback.dump.truncation,tui.exit.scrollback.dump.truncation.long-session-omits-earlier-blocks,tui.exit.scrollback.dump.truncation.short-session-renders-all-blocks]

## 2. Exit path wiring

- [x] I4 Wire `dump_conversation_to_scrollback()` after `restore_terminal()` in `src/modes/interactive.rs`, `src/modes/attach.rs`, and `src/modes/auto_daemon.rs` so every interactive-mode exit path, including `/quit` and `Ctrl-C`, plus attach-mode and auto-daemon-mode exits, all share the same scrollback dump behavior. [covers=tui.exit.scrollback.dump.on.exit,tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation,tui.exit.scrollback.dump.on.exit.all-exit-paths-render]

## 3. Verification

- [x] V1 Positive/negative: add focused settings and guard tests that prove `scrollback_on_exit = false` skips the dump, unset and explicit `true` still allow it, and non-terminal stdout short-circuits without scrollback output. [covers=tui.exit.scrollback.dump.on.exit.non-terminal-stdout-skips-dump,tui.exit.scrollback.dump.setting.opt-out,tui.exit.scrollback.dump.setting.opt-out.disabled-setting-skips-dump,tui.exit.scrollback.dump.setting.opt-out.default-or-true-enables-dump] [evidence=openspec/changes/archive/2026-04-23-tui-exit-scrollback/evidence/settings-and-tty-guard.txt]
- [x] V2 Positive/negative: add scrollback-render tests with mock `ConversationBlock` values that prove block rendering includes separator, bold prompt, assistant markdown, consistent thinking-summary rendering or omission, and tool headers, tool results truncate after 10 lines with an omission indicator, long sessions emit the omission header plus only the last 20 blocks, short sessions emit all blocks with no omission header, and terminal width detection falls back to 80 columns when size lookup fails. [covers=tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation,tui.exit.scrollback.dump.truncation,tui.exit.scrollback.dump.truncation.long-session-omits-earlier-blocks,tui.exit.scrollback.dump.truncation.short-session-renders-all-blocks,tui.exit.scrollback.dump.content.structure,tui.exit.scrollback.dump.content.structure.block-rendering-preserves-prompt-markdown-and-tool-sections,tui.exit.scrollback.dump.content.structure.tool-output-truncates-after-ten-lines] [evidence=openspec/changes/archive/2026-04-23-tui-exit-scrollback/evidence/render-and-truncation.txt]
- [x] V3 Positive/negative: add focused exit-path tests or equivalent seams that prove every interactive-mode exit path, including `/quit` and `Ctrl-C`, plus attach-mode and auto-daemon-mode exit flows all call the shared dump after `restore_terminal()`, and respect the shared non-terminal stdout skip path instead of forcing scrollback output when stdout is redirected. [covers=tui.exit.scrollback.dump.on.exit,tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation,tui.exit.scrollback.dump.on.exit.all-exit-paths-render,tui.exit.scrollback.dump.on.exit.non-terminal-stdout-skips-dump] [evidence=openspec/changes/archive/2026-04-23-tui-exit-scrollback/evidence/exit-path-parity.txt]

## Verification Matrix

- `tui.exit.scrollback.dump.on.exit` -> `I4`, `V3`
- `tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation` -> `I2`, `I4`, `V2`, `V3`
- `tui.exit.scrollback.dump.on.exit.all-exit-paths-render` -> `I4`, `V3`
- `tui.exit.scrollback.dump.on.exit.non-terminal-stdout-skips-dump` -> `I1`, `V1`
- `tui.exit.scrollback.dump.truncation` -> `I3`, `V2`
- `tui.exit.scrollback.dump.truncation.long-session-omits-earlier-blocks` -> `I3`, `V2`
- `tui.exit.scrollback.dump.truncation.short-session-renders-all-blocks` -> `I3`, `V2`
- `tui.exit.scrollback.dump.setting.opt-out` -> `I1`, `V1`
- `tui.exit.scrollback.dump.setting.opt-out.disabled-setting-skips-dump` -> `I1`, `V1`
- `tui.exit.scrollback.dump.setting.opt-out.default-or-true-enables-dump` -> `I1`, `V1`
- `tui.exit.scrollback.dump.content.structure` -> `I2`, `V2`
- `tui.exit.scrollback.dump.content.structure.block-rendering-preserves-prompt-markdown-and-tool-sections` -> `I2`, `V2`
- `tui.exit.scrollback.dump.content.structure.tool-output-truncates-after-ten-lines` -> `I2`, `V2`
