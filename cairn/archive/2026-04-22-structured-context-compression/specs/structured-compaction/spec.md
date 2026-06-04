## ADDED Requirements

### Requirement: Tool-result pruning pre-pass
ID: structured.compaction.pruning.prepass
The system SHALL replace tool result content older than the tail window with informative one-line summaries before LLM summarization. Summaries SHALL include the tool name, key arguments, and a size/outcome indicator. This pre-pass SHALL NOT require an LLM call.

#### Scenario: Bash tool result is pruned
ID: structured.compaction.pruning.prepass.bashtoolresult
- **WHEN** a bash tool result outside the tail window contains command output
- **THEN** it is replaced with a summary like `[bash] ran 'npm test' -> exit 0, 47 lines output`

#### Scenario: Read tool result is pruned
ID: structured.compaction.pruning.prepass.readtoolresult
- **WHEN** a read tool result outside the tail window contains file content
- **THEN** it is replaced with a summary like `[read] read src/main.rs from line 1 (1,200 chars)`

#### Scenario: Common file and search tool results are pruned
ID: structured.compaction.pruning.prepass.commonfiletools
- **WHEN** write, grep/rg, or edit tool results outside the tail window are pruned
- **THEN** each result is replaced with a one-line summary containing the tool name plus key path/pattern and size or count information

#### Scenario: Subagent tool result is pruned
ID: structured.compaction.pruning.prepass.subagenttoolresult
- **WHEN** a subagent tool result outside the tail window is pruned
- **THEN** it is replaced with a one-line summary containing the delegated goal and result size

#### Scenario: Unknown tool type
ID: structured.compaction.pruning.prepass.unknowntooltype
- **WHEN** a tool result for an unrecognized tool name is pruned
- **THEN** a generic summary is generated showing tool name, first argument values, and content size

#### Scenario: Manual /compact reuses pruning path
ID: structured.compaction.pruning.prepass.manualcompactsharedpath
- **WHEN** a user invokes `/compact` in standalone or daemon-backed sessions
- **THEN** older tool results are pruned through the same shared helper used by the pruning pre-pass
- **THEN** recent tail tool results remain intact

---

### Requirement: LLM-powered structured summarization
ID: structured.compaction.structured.summarization
The system SHALL use an auxiliary model to summarize middle-turn messages (between protected head and tail) using a structured template with sections: Active Task, Key Decisions Made, Files Modified, and Remaining Work.

#### Scenario: Compaction triggers summarization
ID: structured.compaction.structured.summarization.triggeredsummary
- **WHEN** context usage exceeds the compaction threshold and an auxiliary model is available
- **THEN** middle messages are sent to the auxiliary model with the structured template
- **THEN** the resulting summary replaces the middle messages in the conversation

#### Scenario: Auxiliary model unavailable
ID: structured.compaction.structured.summarization.auxmodelunavailable
- **WHEN** compaction triggers but no auxiliary model is configured
- **THEN** the system falls back to truncation-only compaction (existing behavior)

#### Scenario: Auxiliary summarization call fails or times out
ID: structured.compaction.structured.summarization.summarycallfallback
- **WHEN** compaction triggers, an auxiliary model is configured, and the summarization call fails or times out
- **THEN** the system falls back to truncation-only compaction instead of blocking the conversation

---

### Requirement: Iterative summary updates
ID: structured.compaction.iterative.summaryupdates
The system SHALL support multiple compaction passes within a single session. When compaction fires after a previous summary exists, the previous summary SHALL be included in the summarization prompt so the model produces a cumulative update. The system SHALL persist the latest compaction summary in recoverable session state so reopened or replayed sessions can seed later compaction passes with the prior summary.

#### Scenario: Second compaction in same session
ID: structured.compaction.iterative.summaryupdates.secondcompaction
- **WHEN** compaction triggers and a previous compaction summary exists
- **THEN** the previous summary is included as context for the new summarization
- **THEN** the new summary incorporates information from both the previous summary and the new middle messages

#### Scenario: Reopened session restores persisted compaction summary
ID: structured.compaction.iterative.summaryupdates.reopenrecovery
- **WHEN** a session with a persisted compaction summary is reopened or replayed for resume
- **THEN** the latest compaction summary is restored from session state for reuse during later compaction passes
- **THEN** the persisted summary remains recoverable from `clankers-db/tool_results` for debugging or recovery flows

---

### Requirement: Token-budget tail protection
ID: structured.compaction.tail.budgetprotection
The system SHALL protect recent messages using a token budget (default 40% of context window) rather than a fixed message count. Messages are selected from most recent backward until the budget is consumed.

#### Scenario: Tail budget adapts to context window
ID: structured.compaction.tail.budgetprotection.contextwindowadapts
- **WHEN** the context window is 200k tokens
- **THEN** the tail window protects approximately 80k tokens of recent messages

#### Scenario: Short recent messages
ID: structured.compaction.tail.budgetprotection.shortrecentmessages
- **WHEN** recent messages are short (few tokens each)
- **THEN** more messages are preserved in the tail compared to fixed-count protection

---

### Requirement: Summary handoff framing
ID: structured.compaction.summary.handoffframing
The system SHALL prefix compaction summaries with a notice that frames the summary as background reference from a previous context window. The notice SHALL instruct the model to respond only to the latest user message, not to questions or tasks mentioned in the summary.

#### Scenario: Model receives compacted context
ID: structured.compaction.summary.handoffframing.modelreceivescompactedcontext
- **WHEN** the model receives a conversation with a compaction summary
- **THEN** the summary is prefixed with a handoff notice
- **THEN** the handoff notice tells the model to treat the summary as background reference and not to re-execute tasks described in it
