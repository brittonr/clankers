# inline-output-mode Specification

## Purpose
TBD - created by archiving change inline-rendering-mode. Update Purpose after archive.
## Requirements
### Requirement: Inline output mode CLI flag
The system SHALL accept `--mode inline` and `--inline` as CLI arguments. When specified with `-p`, the agent SHALL render output using the inline renderer instead of plain text.

#### Scenario: Mode flag
- **WHEN** `clankers -p "hello" --mode inline` is invoked
- **THEN** the agent runs in headless mode with inline styled output

#### Scenario: Shorthand flag
- **WHEN** `clankers -p "hello" --inline` is invoked
- **THEN** the behavior is identical to `--mode inline`

#### Scenario: Without -p
- **WHEN** `clankers --inline` is invoked without `-p`
- **THEN** the interactive TUI launches (inline only applies to headless mode)

### Requirement: Streaming assistant text rendering
The inline renderer SHALL render assistant message text as styled markdown via `InlineMarkdown`. Content SHALL appear incrementally as `MessageUpdate` events arrive.

#### Scenario: Markdown formatting
- **WHEN** the assistant streams `"# Title\n\nSome **bold** text."`
- **THEN** the output shows styled heading, bold text, and paragraph structure

#### Scenario: Incremental rendering
- **WHEN** `MessageUpdate` events arrive with text deltas
- **THEN** each delta triggers a view rebuild and the new content appears immediately

### Requirement: Tool call rendering
The inline renderer SHALL render tool calls with a header showing the tool name and a summary of the input. Tool execution output SHALL render as dimmed monospace text.

#### Scenario: Tool call header
- **WHEN** a `ToolCall` event arrives for tool "bash" with input `{"command": "ls -la"}`
- **THEN** the output shows a styled header like "⚡ bash: ls -la"

#### Scenario: Tool execution output
- **WHEN** `ToolExecutionUpdate` events arrive with partial stdout
- **THEN** the output appears as dimmed text below the tool header

#### Scenario: Tool result
- **WHEN** `ToolExecutionEnd` arrives with the final result
- **THEN** the tool section is finalized (no further updates to it)

### Requirement: Tool error rendering
The inline renderer SHALL render tool errors with a distinct error style (red/bold).

#### Scenario: Tool error
- **WHEN** `ToolExecutionEnd` arrives with `is_error: true`
- **THEN** the result renders in error style distinguishable from success output

### Requirement: Thinking block rendering
The inline renderer SHALL render thinking/extended thinking blocks with a distinct dimmed italic style and a "Thinking..." prefix.

#### Scenario: Thinking content
- **WHEN** `ContentBlockStart` arrives with a thinking content block
- **THEN** the output shows "Thinking..." followed by the thinking text in dimmed italic

### Requirement: Turn boundary rendering
The inline renderer SHALL render a visual separator between agent turns (when the agent loops back after tool results).

#### Scenario: Multi-turn conversation
- **WHEN** `TurnEnd` fires and `TurnStart` fires for the next turn
- **THEN** a horizontal separator or blank line appears between the turns

### Requirement: Usage stats rendering
The inline renderer SHALL render token usage statistics at the end of the run when `--stats` is specified.

#### Scenario: Stats display
- **WHEN** the agent finishes and `--stats` was passed
- **THEN** the output shows input tokens, output tokens, and cost

### Requirement: Terminal width detection
The inline renderer SHALL detect terminal width via `crossterm::terminal::size()`. If detection fails (piped output, no terminal), it SHALL fall back to 80 columns.

#### Scenario: Terminal detected
- **WHEN** stdout is a terminal with width 120
- **THEN** the renderer uses width 120

#### Scenario: Piped output
- **WHEN** stdout is piped (not a terminal)
- **THEN** the renderer uses width 80

### Requirement: Keyed nodes for reconciliation
Each message and tool call SHALL be assigned a stable key for reconciliation. This ensures that when the view tree is rebuilt on each event, existing nodes preserve their state and only new/changed content produces output.

#### Scenario: Stable keys
- **WHEN** the assistant streams 3 messages and the view is rebuilt on each delta
- **THEN** earlier messages are not re-rendered (frame diff produces no output for unchanged cells)

