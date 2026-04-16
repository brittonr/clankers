## ADDED Requirements

### Requirement: Tool-result pruning pre-pass
The system SHALL replace tool result content older than the tail window with informative one-line summaries before LLM summarization. Summaries SHALL include the tool name, key arguments, and a size/outcome indicator. This pre-pass SHALL NOT require an LLM call.

#### Scenario: Bash tool result is pruned
- **WHEN** a bash tool result outside the tail window contains command output
- **THEN** it is replaced with a summary like `[bash] ran 'npm test' -> exit 0, 47 lines output`

#### Scenario: Read tool result is pruned
- **WHEN** a read tool result outside the tail window contains file content
- **THEN** it is replaced with a summary like `[read] read src/main.rs from line 1 (1,200 chars)`

#### Scenario: Unknown tool type
- **WHEN** a tool result for an unrecognized tool name is pruned
- **THEN** a generic summary is generated showing tool name, first argument values, and content size

---

### Requirement: LLM-powered structured summarization
The system SHALL use an auxiliary model to summarize middle-turn messages (between protected head and tail) using a structured template with sections: Active Task, Key Decisions Made, Files Modified, and Remaining Work.

#### Scenario: Compaction triggers summarization
- **WHEN** context usage exceeds the compaction threshold and an auxiliary model is available
- **THEN** middle messages are sent to the auxiliary model with the structured template
- **THEN** the resulting summary replaces the middle messages in the conversation

#### Scenario: Auxiliary model unavailable
- **WHEN** compaction triggers but no auxiliary model is configured
- **THEN** the system falls back to truncation-only compaction (existing behavior)

---

### Requirement: Iterative summary updates
The system SHALL support multiple compaction passes within a single session. When compaction fires after a previous summary exists, the previous summary SHALL be included in the summarization prompt so the model produces a cumulative update.

#### Scenario: Second compaction in same session
- **WHEN** compaction triggers and a previous compaction summary exists
- **THEN** the previous summary is included as context for the new summarization
- **THEN** the new summary incorporates information from both the previous summary and the new middle messages

---

### Requirement: Token-budget tail protection
The system SHALL protect recent messages using a token budget (default 40% of context window) rather than a fixed message count. Messages are selected from most recent backward until the budget is consumed.

#### Scenario: Tail budget adapts to context window
- **WHEN** the context window is 200k tokens
- **THEN** the tail window protects approximately 80k tokens of recent messages

#### Scenario: Short recent messages
- **WHEN** recent messages are short (few tokens each)
- **THEN** more messages are preserved in the tail compared to fixed-count protection

---

### Requirement: Summary handoff framing
The system SHALL prefix compaction summaries with a notice that frames the summary as background reference from a previous context window. The notice SHALL instruct the model to respond only to the latest user message, not to questions or tasks mentioned in the summary.

#### Scenario: Model receives compacted context
- **WHEN** the model receives a conversation with a compaction summary
- **THEN** the summary is prefixed with a handoff notice
- **THEN** the model does not re-execute tasks described in the summary
