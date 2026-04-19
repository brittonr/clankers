# context-compression Specification

## Purpose
TBD - created by archiving change agent-learning-loop. Update Purpose after archive.
## Requirements
### Requirement: Agent can compress conversation context

The agent SHALL be able to compress the current conversation by calling the `compress` tool. Compression sends the message history through a summarization model and replaces it with the summary, preserving semantic content while reducing token count.

#### Scenario: Compress a long conversation
- **WHEN** the agent calls `compress` with no arguments and the conversation has 20+ messages
- **THEN** the tool sends the message history to a fast/cheap model for summarization, replaces the messages with a single system-injected summary message, and returns the before/after token counts

#### Scenario: Compress with too few messages
- **WHEN** the agent calls `compress` and the conversation has fewer than 5 messages
- **THEN** the tool returns an error indicating compression is not useful yet

### Requirement: Slash command triggers compression

The user SHALL be able to trigger compression via `/compress` in the TUI. This calls the same compression logic as the tool.

#### Scenario: User triggers compression
- **WHEN** the user types `/compress`
- **THEN** the conversation is summarized and replaced, with a system message showing before/after token counts

### Requirement: Compression preserves recent context

Compression SHALL keep the most recent N messages intact (default: 4) and only summarize older messages. This preserves the immediate working context while compressing history.

#### Scenario: Recent messages preserved
- **WHEN** compression runs on a 30-message conversation with `keep_recent: 4`
- **THEN** the last 4 messages remain unchanged, messages 1-26 are replaced by a summary

### Requirement: Compression uses a configurable model

The model used for summarization SHALL be configurable via `settings.yaml` under `compression.model`. The default SHALL be the cheapest available model from the configured provider (e.g., `haiku` for Anthropic, `gpt-4o-mini` for OpenAI).

#### Scenario: Custom compression model
- **WHEN** `settings.yaml` contains `compression: { model: "claude-haiku-4-20250514" }`
- **THEN** compression uses that model for summarization

#### Scenario: Default model
- **WHEN** no compression model is configured
- **THEN** the system selects the cheapest model from the active provider

### Requirement: Summary format is structured

The compression summary SHALL be formatted as a structured block injected as a system message. It includes: a header identifying it as a compressed summary, the key topics discussed, decisions made, files modified, and any unresolved items.

#### Scenario: Summary content
- **WHEN** compression completes
- **THEN** the summary message contains sections for: topics covered, decisions made, files touched, and open threads

### Requirement: Compression reports savings

After compression, the tool SHALL report the token count before and after, and the percentage reduction.

#### Scenario: Savings reported
- **WHEN** compression completes
- **THEN** the response includes `{"before_tokens": 45000, "after_tokens": 8000, "reduction": "82%"}`

### Requirement: Auto-compression nudge

When the context window reaches 80% capacity, the system SHALL inject a one-line nudge into the assistant's context suggesting `/compress` or the compress tool. This is advisory, not automatic.

#### Scenario: Nudge at threshold
- **WHEN** the estimated token count exceeds 80% of the model's context window during context building
- **THEN** a brief note is appended to the system prompt: "Context is at {pct}% capacity. Consider using the compress tool to summarize older messages."

#### Scenario: No nudge below threshold
- **WHEN** the estimated token count is below 80% of the model's context window
- **THEN** no nudge is added

