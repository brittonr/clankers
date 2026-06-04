## ADDED Requirements

### Requirement: Conversation dumped to scrollback on TUI exit
ID: tui.exit.scrollback.dump.on.exit
When the TUI exits, the system SHALL render the conversation to terminal scrollback so the user can scroll up to review it.

#### Scenario: Normal exit renders the conversation
ID: tui.exit.scrollback.dump.on.exit.normal-exit-renders-conversation
- **WHEN** the user exits the TUI via `/quit` or Ctrl-C
- **THEN** the conversation MUST appear in terminal scrollback with styled markdown, tool call headers, and user prompts

#### Scenario: All exit paths render
ID: tui.exit.scrollback.dump.on.exit.all-exit-paths-render
- **WHEN** the TUI exits from interactive mode, attach mode, or auto-daemon mode
- **THEN** the scrollback dump MUST occur in all three paths

#### Scenario: Non-terminal stdout skips the dump
ID: tui.exit.scrollback.dump.on.exit.non-terminal-stdout-skips-dump
- **WHEN** stdout is not a terminal because output is piped or redirected
- **THEN** the scrollback dump SHALL be skipped

### Requirement: Scrollback dump truncation
ID: tui.exit.scrollback.dump.truncation
For long sessions, the dump SHALL truncate to avoid flooding scrollback.

#### Scenario: Long session omits earlier blocks
ID: tui.exit.scrollback.dump.truncation.long-session-omits-earlier-blocks
- **WHEN** the conversation has more than 20 blocks
- **THEN** only the last 20 blocks MUST be rendered
- **AND** the dump MUST include a line indicating how many earlier blocks were omitted

#### Scenario: Short session renders all blocks
ID: tui.exit.scrollback.dump.truncation.short-session-renders-all-blocks
- **WHEN** the conversation has 20 or fewer blocks
- **THEN** all conversation blocks MUST be rendered
- **AND** no omission header may appear

### Requirement: Scrollback dump opt-out
ID: tui.exit.scrollback.dump.setting.opt-out
The user SHALL be able to disable the scrollback dump via settings while the default behavior remains enabled when the setting is unset.

#### Scenario: Disabled setting skips the dump
ID: tui.exit.scrollback.dump.setting.opt-out.disabled-setting-skips-dump
- **WHEN** `scrollback_on_exit` is `false` in settings
- **THEN** the dump MUST be skipped and the TUI MUST exit silently

#### Scenario: Default or true enables the dump
ID: tui.exit.scrollback.dump.setting.opt-out.default-or-true-enables-dump
- **WHEN** `scrollback_on_exit` is unset or explicitly `true`
- **THEN** the dump MUST occur normally

### Requirement: Scrollback content structure
ID: tui.exit.scrollback.dump.content.structure
Each conversation block SHALL render with its user prompt, assistant responses, and tool activity in a readable scrollback-friendly structure.

#### Scenario: Block rendering preserves prompt, markdown, and tool sections
ID: tui.exit.scrollback.dump.content.structure.block-rendering-preserves-prompt-markdown-and-tool-sections
- **WHEN** a block contains a user prompt, assistant markdown, and tool calls
- **THEN** the scrollback MUST show a separator with timestamp, a bold user prompt, a styled markdown response, and tool call headers with dimmed output
- **AND** adjacent blocks MUST be separated by blank lines

#### Scenario: Tool output truncates after ten lines
ID: tui.exit.scrollback.dump.content.structure.tool-output-truncates-after-ten-lines
- **WHEN** a tool result has more than 10 lines
- **THEN** only the first 10 lines MUST be shown
- **AND** the output MUST include an omission indicator
