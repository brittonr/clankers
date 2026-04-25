## ADDED Requirements

### Requirement: Retry and stop-policy decisions MUST be engine-owned for the next executable turn slice
The system MUST make `clankers-engine` the authoritative owner for retry decisions, retry budget state, model-continuation turn budget, token-limit terminalization, and terminal stop policy in the next executable engine slice. Provider I/O, provider-specific error classification, and actual waiting remain host-shell responsibilities, but hosts MUST follow engine-owned effects and outcomes instead of re-deriving retry authorization, retry count, retry delay, budget exhaustion, or terminal stop decisions locally.
r[embeddable-agent-engine.retry-stop-policy-owned]

#### Scenario: host classification is input and engine retry authorization is policy
r[embeddable-agent-engine.host-classification]
- **WHEN** the host reports a model failure for a pending engine model request
- **THEN** the host feedback includes engine-native failure input fields: the pending model request identity, failure `message`, optional provider/status code `status`, and provider-classified `retryable` flag
- **THEN** the original structured `AgentError` remains host-side data correlated with the pending model request identity rather than an engine payload field
- **THEN** the engine decides whether that classified failure is authorized to retry under the current phase, correlation ID, retry budget, and turn state
- **THEN** the host does not independently decide whether the classified failure should retry after it has been submitted to the engine

#### Scenario: retryable model failure schedules engine-owned retry work
r[embeddable-agent-engine.retry-scheduling]
- **WHEN** the host reports a retryable model failure for a pending engine model request and the engine retry budget permits another attempt
- **THEN** the engine records the retry attempt against engine-owned state for that pending model request
- **THEN** the engine moves to a retry-waiting phase and returns `EngineEffect::ScheduleRetry { request_id, delay }` carrying the same pending model request identity and backoff delay that the host must wait on
- **THEN** after the delay completes, the host reports an engine-native retry-ready input and the engine re-emits the model request for the same pending model request identity
- **THEN** the host does not calculate retry count, backoff delay, or retry request timing outside the engine boundary

#### Scenario: retry budget scope resets on successful model feedback
r[embeddable-agent-engine.retry-budget-reset]
- **WHEN** the engine creates a pending model request for the migrated slice
- **THEN** that pending model request receives its own retry budget and retry-attempt counter
- **THEN** retry attempts for that request consume only that request's retry budget
- **THEN** retry effects for that request preserve the same pending model request identity rather than minting a fresh request identity
- **THEN** a successful model completion clears the retry-attempt counter for that pending request
- **THEN** any later follow-up model request minted after tool feedback receives a fresh retry budget

#### Scenario: deterministic retry defaults preserve current behavior
r[embeddable-agent-engine.retry-defaults]
- **WHEN** the engine uses the default turn-level retry policy for the migrated slice
- **THEN** the policy allows at most two additional attempts after the initial model attempt
- **THEN** the default backoff delays are 1 second before the first retry and 4 seconds before the second retry
- **THEN** the default policy uses no jitter and emits no retry delay after the retry budget is exhausted

#### Scenario: non-retryable model failure terminalizes through engine policy
r[embeddable-agent-engine.non-retryable-terminalization]
- **WHEN** the host reports a non-retryable model failure for a pending engine model request
- **THEN** the engine clears pending model work and emits terminal output in this order: `BusyChanged { busy: false }`
- **THEN** the engine records `EngineOutcome.terminal_failure = Some(EngineTerminalFailure { message, status, retryable })` using the latest host-supplied failure details
- **THEN** the engine emits `Notice` carrying the failure message for host error reporting
- **THEN** the engine emits `TurnFinished { stop_reason: StopReason::Stop }`
- **THEN** the host does not synthesize its own terminal state for that failure path

#### Scenario: exhausted retry budget terminalizes through engine policy
r[embeddable-agent-engine.retry-exhaustion-terminalization]
- **WHEN** retryable model failures exceed the engine-owned retry budget
- **THEN** the engine clears pending model work and emits terminal output in this order: `BusyChanged { busy: false }`
- **THEN** the engine records `EngineOutcome.terminal_failure = Some(EngineTerminalFailure { message, status, retryable })` using the latest host-supplied failure details
- **THEN** the engine emits `Notice` carrying the latest failure message for host error reporting
- **THEN** the engine emits `TurnFinished { stop_reason: StopReason::Stop }`
- **THEN** no additional model request is emitted for that failed turn

#### Scenario: model-continuation budget has explicit counting rules
r[embeddable-agent-engine.model-continuation-budget]
- **WHEN** the engine evaluates the model-continuation budget for a submitted turn
- **THEN** the budget MUST be at least one model request slot or prompt submission is rejected without effects
- **THEN** the initial model request consumes one model request slot from that budget
- **THEN** each follow-up model request minted after tool feedback consumes one additional model request slot from that budget
- **THEN** retries of the same pending model request do not consume additional model request slots because they are governed by the separate retry budget
- **THEN** the default Clankers adapters preserve the current normal-turn budget of 25 total model request slots and orchestration follow-up phase budget of 10 total model request slots by passing those values into engine configuration through named constants
- **THEN** both values are total per-turn/per-phase slots that include the initial model request for that turn or orchestration phase, not additional follow-up slots after the initial request

#### Scenario: zero model-continuation budget is rejected before initial request
r[embeddable-agent-engine.zero-budget]
- **WHEN** a host submits a prompt with a zero model-continuation budget for the migrated slice
- **THEN** the engine returns `EngineOutcome { rejection: Some(EngineRejection::InvalidBudget), effects: [], terminal_failure: None }`
- **THEN** the engine leaves state unchanged and emits no model request, retry, tool, semantic event, or terminal turn effects
- **THEN** the host surfaces that rejection through the existing engine-rejection error path rather than starting a turn

#### Scenario: turn budget blocks unbounded continuations
r[embeddable-agent-engine.budget-exhaustion]
- **WHEN** tool feedback would otherwise request another model continuation after the engine-owned model-continuation budget for the turn is exhausted
- **THEN** the engine first records all accepted tool feedback for that step in canonical engine messages
- **THEN** the engine clears pending tool work and emits terminal output in this order: `BusyChanged { busy: false }`
- **THEN** the engine emits `Notice { message: "engine model request slot budget exhausted" }`
- **THEN** the engine emits `TurnFinished { stop_reason: StopReason::Stop }`
- **THEN** no model request effect is emitted for that exhausted turn
- **THEN** app shells do not enforce a second independent max-turn policy for the migrated slice

#### Scenario: retry-wait cancellation clears scheduled retry work
r[embeddable-agent-engine.retry-wait-cancellation]
- **WHEN** the host reports `CancelTurn { reason: "turn cancelled" }` while the engine is waiting for retry-ready feedback after `EngineEffect::ScheduleRetry`
- **THEN** the engine clears the pending model request and retry-wait state
- **THEN** the engine emits cancellation terminal output in this order: `BusyChanged { busy: false }`, `Notice { message: "turn cancelled" }`, then `TurnFinished { stop_reason: StopReason::Stop }`
- **THEN** later retry-ready, model-success, or model-failure feedback for the cancelled request is rejected without state mutation or effects

#### Scenario: token-limit stop is explicit engine terminal behavior
r[embeddable-agent-engine.max-tokens]
- **WHEN** a model completion returns assistant content with `StopReason::MaxTokens` for the migrated slice
- **THEN** the engine accepts that model completion by appending the assistant content to canonical engine messages
- **THEN** the engine clears pending model work and terminalizes the turn without emitting retry, tool, or follow-up model effects
- **THEN** the engine emits the same terminal event ordering as other successful terminal stops, including `BusyChanged { busy: false }` before `TurnFinished { stop_reason: StopReason::MaxTokens }`
- **THEN** the host does not collapse token-limit handling into an untested generic stop branch or auto-continue without a future spec change

#### Scenario: retry and budget effect payloads stay engine-native
r[embeddable-agent-engine.engine-native-payloads]
- **WHEN** the engine emits retry, budget, token-limit, or terminal effects for this slice
- **THEN** those effect payloads use engine-owned plain data such as `EngineCorrelationId`, `EngineEffect::ScheduleRetry { request_id, delay }`, engine retry policy fields, engine delay values, engine semantic events, and `EngineModelRequest`
- **THEN** the engine does not introduce provider-shaped `CompletionRequest` payloads, daemon protocol types, TUI types, Tokio handles, timestamps, shell-generated message IDs, or shell-specific request construction into the retry/budget/stop-policy surface

### Requirement: Adapter parity rails MUST cover retry, budget, and token-limit ownership
The system MUST verify that Clankers runtime adapters execute engine-owned retry, budget, and token-limit effects without retaining an authoritative copy of the migrated policy in async shell code.
r[embeddable-agent-engine.adapter-parity-rails]

#### Scenario: reducer tests cover positive and negative retry policy paths
r[embeddable-agent-engine.reducer-retry-tests]
- **WHEN** validation runs for this slice
- **THEN** engine reducer tests cover retryable failure scheduling, default 1-second and 4-second retry delays, non-retryable failure terminalization, retry exhaustion, preserved correlation IDs, and no message mutation on failed retry attempts
- **THEN** negative tests cover mismatched request IDs, wrong-phase retry feedback, duplicate failure feedback, and invalid retry after terminalization

#### Scenario: reducer tests cover turn budget and token-limit terminal paths
r[embeddable-agent-engine.reducer-budget-token-tests]
- **WHEN** validation runs for this slice
- **THEN** engine reducer tests cover initial request budget counting, retry attempts not consuming continuation budget, continuation within budget, budget exhaustion terminalization, and `StopReason::MaxTokens` terminalization
- **THEN** the tests assert deterministic state, effects, semantic events, and terminal effect ordering for each terminal path

#### Scenario: invalid retry feedback leaves state unchanged
r[embeddable-agent-engine.invalid-retry-feedback]
- **WHEN** the host reports retry failure, retry-ready, or model feedback with a mismatched request ID, in a phase where that feedback is not valid, or after terminalization
- **THEN** model-success or model-failure feedback while the engine is waiting for retry-ready feedback is rejected until a matching retry-ready input is accepted
- **THEN** matching retry-ready input is valid only in the retry-waiting phase and re-emits the model request as specified by the retry scheduling scenario
- **THEN** the engine returns an explicit rejection such as `EngineRejection::CorrelationMismatch` for wrong IDs or `EngineRejection::InvalidPhase` for wrong-phase and post-terminal feedback
- **THEN** the engine leaves state unchanged and emits no effects

#### Scenario: runtime adapter rails reject local policy re-derivation
r[embeddable-agent-engine.adapter-rail]
- **WHEN** validation runs after adapter migration
- **THEN** a deterministic static FCIS-style source inventory covers non-test `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}` and fails if those files declare retry-budget or retry-backoff constants, perform arithmetic to choose retry delays, loop or branch over `config.max_turns` to decide continuation, or branch on `StopReason::MaxTokens` for terminal policy outside engine effect interpretation
- **THEN** focused runtime adapter tests prove shell-visible retry, cancellation, budget exhaustion, zero-budget rejection, token-limit terminalization, and terminal behavior remains unchanged while using engine-owned decisions
- **THEN** the static rail allows shell-only matching on `EngineEffect::ScheduleRetry`, sleeping for an engine-provided delay, executing `EngineEffect::RequestModel`, parsing provider stop strings into `StopReason`, provider request conversion in `turn/execution.rs`, and named adapter constants in `crates/clankers-agent/src/lib.rs` that only pass existing normal/orchestration budgets into engine configuration
