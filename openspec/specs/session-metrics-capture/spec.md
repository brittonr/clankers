## ADDED Requirements

### Requirement: Sessions emit unified metrics events
Clankers MUST translate runtime activity into a unified metrics stream that covers session lifecycle, turn lifecycle, model changes, session compaction, cancellation, and process-monitoring signals when procmon is active.

#### Scenario: Session summary captures model switches and compaction
- **GIVEN** a session that changes models and compacts old context
- **WHEN** the session ends
- **THEN** the session metrics summary records wall time, turn count, model switch count, compaction count, and total tokens saved by compaction

#### Scenario: Procmon metrics are optional
- **GIVEN** process monitoring is disabled for a session
- **WHEN** no procmon events are emitted
- **THEN** metrics capture skips process fields without treating the session as failed

### Requirement: Tool execution metrics are recorded per call
Clankers MUST record one tool metric entry per execution with the tool name, call ID, source, duration, outcome, and streamed-result totals.

#### Scenario: Built-in tool execution succeeds
- **GIVEN** the agent calls a built-in tool such as `read`
- **WHEN** the tool finishes successfully
- **THEN** metrics capture records the tool name, `builtin` source, duration, success outcome, and streamed byte/chunk totals for that call

#### Scenario: Tool execution is blocked or errors
- **GIVEN** a tool call is blocked by the sandbox or returns an error
- **WHEN** execution ends
- **THEN** metrics capture records the tool outcome as blocked or error and increments the session error counters

#### Scenario: Plugin tool execution carries plugin ownership
- **GIVEN** the agent calls a plugin-provided tool
- **WHEN** metrics capture records the call
- **THEN** the entry records the tool source as `plugin` and includes the owning plugin name

### Requirement: Plugin activity metrics are recorded
Clankers MUST record plugin lifecycle and activity metrics, including load results, event dispatches, hook denials, tool calls, UI actions, and plugin execution errors.

#### Scenario: Plugin handles an agent event and emits UI actions
- **GIVEN** a plugin subscribes to `tool_call`
- **WHEN** the daemon or standalone runtime dispatches that event and the plugin returns a status update or widget action
- **THEN** metrics capture increments that plugin's event-dispatch and UI-action counters

#### Scenario: Plugin hook denies an operation
- **GIVEN** a plugin denies a pre-tool hook
- **WHEN** the denial is surfaced back to the runtime
- **THEN** metrics capture records a hook-deny event for that plugin and increments the session deny counter

#### Scenario: Plugin load fails
- **GIVEN** plugin startup fails for a discovered plugin
- **WHEN** the runtime logs the failure
- **THEN** metrics capture records the plugin name, failure outcome, and failure count without stopping other plugins from being counted

### Requirement: Token and cost metrics are recorded by model
Clankers MUST record per-turn and per-session token usage by model, including input, output, cache-read, cache-write, and estimated cost when pricing data exists.

#### Scenario: Usage update contributes to the active model
- **GIVEN** the provider reports input, output, and cache tokens for a turn
- **WHEN** metrics capture processes the usage update
- **THEN** the current model's counters increase for those token classes and the session summary updates its cumulative totals

#### Scenario: Model change splits usage across models
- **GIVEN** a session switches from one model to another mid-run
- **WHEN** later usage updates arrive
- **THEN** metrics capture stores per-model counters separately and preserves the total session counters across both models
