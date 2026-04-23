# context-compression Specification

## Purpose
Define how clankers compresses long conversation history while preserving recent working context, structured handoff state, and recoverable summary information across manual and automatic compaction flows.
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

### Requirement: Tool-result pruning pre-pass
The system SHALL replace tool result content older than the protected tail window with informative one-line summaries before LLM summarization. Summaries SHALL include the tool name, key arguments, and a size or outcome indicator, and this pre-pass SHALL NOT require an LLM call.

#### Scenario: Bash tool result is pruned
- **WHEN** a bash tool result outside the tail window contains command output
- **THEN** it is replaced with a summary like `[bash] ran 'npm test' -> exit 0, 47 lines output`

#### Scenario: Read tool result is pruned
- **WHEN** a read tool result outside the tail window contains file content
- **THEN** it is replaced with a summary like `[read] read src/main.rs from line 1 (1,200 chars)`

#### Scenario: Common file and search tool results are pruned
- **WHEN** write, grep/rg, or edit tool results outside the tail window are pruned
- **THEN** each result is replaced with a one-line summary containing the tool name plus key path or pattern and size or count information

#### Scenario: Subagent tool result is pruned
- **WHEN** a subagent tool result outside the tail window is pruned
- **THEN** it is replaced with a one-line summary containing the delegated goal and result size

#### Scenario: Unknown tool type
- **WHEN** a tool result for an unrecognized tool name is pruned
- **THEN** a generic summary is generated showing tool name, first argument values, and content size

#### Scenario: Manual /compact reuses pruning path
- **WHEN** a user invokes `/compact` in standalone or daemon-backed sessions
- **THEN** older tool results are pruned through the same shared helper used by the pruning pre-pass
- **THEN** recent tail tool results remain intact

### Requirement: LLM-powered structured summarization
The system SHALL use an auxiliary model to summarize middle-turn messages between the retained leading context and protected tail using a structured template with sections: Active Task, Key Decisions Made, Files Modified, and Remaining Work.

#### Scenario: Compaction triggers summarization
- **WHEN** context usage exceeds the compaction threshold and an auxiliary model is available
- **THEN** middle messages are sent to the auxiliary model with the structured template
- **THEN** the resulting summary replaces the middle messages in the conversation

#### Scenario: Auxiliary model unavailable
- **WHEN** compaction triggers but no auxiliary model is configured
- **THEN** the system falls back to truncation-only compaction

#### Scenario: Auxiliary summarization call fails or times out
- **WHEN** compaction triggers, an auxiliary model is configured, and the summarization call fails or times out
- **THEN** the system falls back to truncation-only compaction instead of blocking the conversation

### Requirement: Iterative summary updates
The system SHALL support multiple compaction passes within a single session. When compaction fires after a previous summary exists, the previous summary SHALL be included in the summarization prompt so the model produces a cumulative update. The system SHALL persist the latest compaction summary in recoverable session state so reopened or replayed sessions can seed later compaction passes with the prior summary.

#### Scenario: Second compaction in same session
- **WHEN** compaction triggers and a previous compaction summary exists
- **THEN** the previous summary is included as context for the new summarization
- **THEN** the new summary incorporates information from both the previous summary and the new middle messages

#### Scenario: Reopened session restores persisted compaction summary
- **WHEN** a session with a persisted compaction summary is reopened or replayed for resume
- **THEN** the latest compaction summary is restored from session state for reuse during later compaction passes
- **THEN** the persisted summary remains recoverable from `clankers-db/tool_results` for debugging or recovery flows

### Requirement: Token-budget tail protection
The system SHALL protect recent messages using a token budget, defaulting to 40% of the context window, rather than a fixed message count. Messages are selected from most recent backward until the budget is consumed.

#### Scenario: Tail budget adapts to context window
- **WHEN** the context window is 200k tokens
- **THEN** the tail window protects approximately 80k tokens of recent messages

#### Scenario: Short recent messages
- **WHEN** recent messages are short
- **THEN** more messages are preserved in the tail compared to fixed-count protection

### Requirement: Summary handoff framing
The system SHALL prefix compaction summaries with a notice that frames the summary as background reference from a previous context window. The notice SHALL instruct the model to respond only to the latest user message, not to questions or tasks mentioned in the summary.

#### Scenario: Model receives compacted context
- **WHEN** the model receives a conversation with a compaction summary
- **THEN** the summary is prefixed with a handoff notice
- **THEN** the handoff notice tells the model to treat the summary as background reference and not to re-execute tasks described in it

### Requirement: Auto-compression nudge

When the context window reaches 80% capacity, the system SHALL inject a one-line nudge into the assistant's context suggesting `/compress` or the compress tool. This is advisory, not automatic.

#### Scenario: Nudge at threshold
- **WHEN** the estimated token count exceeds 80% of the model's context window during context building
- **THEN** a brief note is appended to the system prompt: "Context is at {pct}% capacity. Consider using the compress tool to summarize older messages."

#### Scenario: No nudge below threshold
- **WHEN** the estimated token count is below 80% of the model's context window
- **THEN** no nudge is added

