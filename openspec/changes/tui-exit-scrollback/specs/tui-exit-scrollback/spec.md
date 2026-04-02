## ADDED Requirements

### Requirement: Conversation dumped to scrollback on TUI exit
When the TUI exits, the system SHALL render the conversation to terminal scrollback so the user can scroll up to review it.

#### Scenario: Normal exit
- **WHEN** the user exits the TUI via `/quit` or Ctrl-C
- **THEN** the conversation appears in terminal scrollback with styled markdown, tool call headers, and user prompts

#### Scenario: All exit paths
- **WHEN** the TUI exits from interactive mode, attach mode, or auto-daemon mode
- **THEN** the scrollback dump occurs in all three paths

#### Scenario: Piped stdout
- **WHEN** stdout is not a terminal (piped or redirected)
- **THEN** the scrollback dump SHALL be skipped

### Requirement: Scrollback dump truncation
For long sessions, the dump SHALL truncate to avoid flooding scrollback.

#### Scenario: Long session
- **WHEN** the conversation has more than 20 blocks
- **THEN** only the last 20 blocks are rendered, preceded by a line indicating how many blocks were omitted

#### Scenario: Short session
- **WHEN** the conversation has 20 or fewer blocks
- **THEN** all blocks are rendered

### Requirement: Scrollback dump opt-out
The user SHALL be able to disable the scrollback dump via settings.

#### Scenario: Setting disabled
- **WHEN** `scrollback_on_exit` is `false` in settings
- **THEN** the dump is skipped and the TUI exits silently

#### Scenario: Setting enabled (default)
- **WHEN** `scrollback_on_exit` is not set or is `true`
- **THEN** the dump occurs normally

### Requirement: Scrollback content structure
Each conversation block SHALL render with its user prompt, assistant responses, and tool activity.

#### Scenario: Block rendering
- **WHEN** a block contains a user prompt, assistant markdown, and tool calls
- **THEN** the scrollback shows: separator with timestamp, bold user prompt, styled markdown response, tool call headers with dimmed output

#### Scenario: Tool output truncation
- **WHEN** a tool result has more than 10 lines
- **THEN** only the first 10 lines are shown with an omission indicator
