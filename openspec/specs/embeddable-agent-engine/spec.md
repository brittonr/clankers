# embeddable-agent-engine Specification

## Purpose

Define the reusable `clankers-engine` boundary for host-facing model/tool turn semantics that compose alongside pure core reducers through adapter seams and sit below Clankers-specific shells such as daemon, TUI, interactive mode, provider runtime, and prompt assembly.
## Requirements
### Requirement: Clankers MUST provide a reusable embeddable engine crate

The system MUST provide a workspace crate named `clankers-engine` that exposes a host-facing agent harness API alongside `clankers-core` and below Clankers-specific shells such as daemon, TUI, interactive mode, and system-prompt assembly. `clankers-core` and `clankers-engine` MUST compose through explicit adapter seams rather than implicit state pass-through.
r[embeddable-agent-engine.reusable-engine-crate]

#### Scenario: engine crate is layered alongside clankers-core through explicit ownership
r[embeddable-agent-engine.core-engine-explicit-layering]

- **WHEN** another Rust project depends on `clankers-engine`
- **THEN** it can drive model/tool turn execution through engine-owned state, inputs, effects, and outcomes without depending on daemon protocol, TUI state, or interactive mode modules
- **THEN** prompt lifecycle, queued prompt replay, loop, thinking, and disabled-tool filter policy remain owned by `clankers-core` and are sequenced with engine turn execution by adapter code
- **THEN** model/tool turn policy does not move downward into `clankers-core` unless a later change adds explicit no-std-core contracts, state, tests, and rails for that migration
- **THEN** the public engine boundary uses engine-native plain-data types rather than `DaemonEvent`, `SessionCommand`, or other Clankers app protocol types

### Requirement: The engine API MUST expose explicit host-driven execution contracts

The engine MUST define explicit host-facing contracts for model execution requests, tool execution requests, host feedback, and semantic engine events after an adapter has accepted any core-owned prompt lifecycle transition.
r[embeddable-agent-engine.host-driven-contracts]

#### Scenario: host submits an accepted prompt through engine input
r[embeddable-agent-engine.accepted-prompt-engine-input]

- **WHEN** `clankers-core` accepts a prompt lifecycle or follow-up transition and controller adapter code normalizes it into engine prompt data
- **THEN** the adapter submits that accepted prompt to `clankers-engine` through engine-native input
- **THEN** `clankers-engine` owns pending model work, model/tool turn state, and continuation from that point forward
- **THEN** `clankers-engine` does not decide queued prompt replay, loop follow-up dispatch, auto-test follow-up dispatch, thinking-level updates, or disabled-tool filter state

#### Scenario: model execution is requested through explicit engine effects
r[embeddable-agent-engine.model-execution-effects-retained]

- **WHEN** the engine decides that model completion work is needed after accepted prompt submission
- **THEN** it emits an explicit model-request effect containing the correlated request identity and the model request payload
- **THEN** the host returns the model completion or model failure through a correlated engine input rather than the engine performing provider I/O directly

#### Scenario: tool execution is requested through explicit engine effects
r[embeddable-agent-engine.tool-execution-effects-retained]

- **WHEN** the engine decides that tool execution work is needed after model feedback requests tools
- **THEN** it emits an explicit tool-execution effect containing the correlated call identity, tool name, and structured tool input payload
- **THEN** the host returns the tool result or tool failure through a correlated engine input rather than the engine invoking tools directly

#### Scenario: semantic engine events stay separate from lifecycle events
r[embeddable-agent-engine.turn-events-lifecycle-events-separated]

- **WHEN** the engine surfaces model/tool turn progress, retry notices, cancellation, or terminal turn outcomes
- **THEN** it emits engine-native semantic events for those turn concerns
- **THEN** loop-state changes, queued-prompt replay, and session prompt-lifecycle busy changes remain core-owned lifecycle outputs translated by adapters outside the engine
- **THEN** engine turn busy and terminal `BusyChanged` events for accepted model/tool/retry work remain engine-owned turn outputs

### Requirement: Turn orchestration MUST be engine-owned reusable policy

The reusable engine boundary MUST own model/tool turn orchestration after prompt lifecycle acceptance, while core-owned lifecycle and follow-up dispatch policy remains outside the engine.
r[embeddable-agent-engine.turn-orchestration-owned-after-acceptance]

#### Scenario: accepted prompt to model to tool to continuation flow is engine-owned
r[embeddable-agent-engine.accepted-prompt-turn-flow-owned]

- **WHEN** adapter code submits an accepted user prompt or accepted follow-up prompt to the engine
- **THEN** the engine owns the state machine that decides model request, tool-call planning, tool feedback ingestion, continuation, retry, cancellation, and terminal turn outcomes
- **THEN** the engine does not own whether the original prompt or follow-up was lifecycle-valid; that decision remains in `clankers-core`

### Requirement: Message evolution policy MUST be reusable through the engine

The engine MUST own the reusable conversation/message evolution rules needed for an embedded harness.

#### Scenario: engine appends canonical conversation entries

- **WHEN** the host submits user input, the engine receives model output, or the host returns tool results
- **THEN** the engine updates canonical conversation state using engine-owned message evolution rules
- **THEN** embedders do not need to reconstruct Clankers-specific assistant, tool-result, or continuation ordering themselves

#### Scenario: message policy remains independent of prompt assembly

- **WHEN** a host supplies its own system prompt or prompt-building policy
- **THEN** the engine still preserves the same message evolution and turn-orchestration contracts
- **THEN** system-prompt discovery from AGENTS.md, OpenSpec context, skills, or project files remains outside the engine boundary

### Requirement: Controller and agent shells MUST become adapters over the engine

Clankers-specific controller and agent shells MUST consume the engine as an adapter layer rather than remaining the authoritative source of reusable harness semantics.

#### Scenario: controller stops owning reusable orchestration policy

- **WHEN** the controller handles prompt, model, tool, retry, or continuation flows that are in scope for the embeddable engine
- **THEN** it translates between session/daemon shell concerns and engine inputs/effects
- **THEN** it does not remain the only authoritative implementation of those reusable orchestration rules

#### Scenario: agent runtime stops owning reusable turn policy

- **WHEN** the agent runtime performs provider calls, tool execution, hook dispatch, or event streaming
- **THEN** it acts as a host/runtime adapter around engine requests and outcomes
- **THEN** reusable turn policy lives in the engine boundary instead of inside async runtime code in `clankers-agent`

### Requirement: App-specific concerns MUST stay outside the engine boundary

The embeddable engine MUST not absorb Clankers application concerns that are not required for a minimal host-facing harness.

#### Scenario: system prompt assembly stays app-specific

- **WHEN** Clankers loads AGENTS.md, SYSTEM.md, APPEND_SYSTEM.md, OpenSpec context, or skill definitions
- **THEN** that behavior remains outside the engine boundary
- **THEN** the engine accepts already-prepared prompt inputs or explicit prompt-policy configuration rather than reading project context files directly

#### Scenario: transport and UI concerns stay app-specific

- **WHEN** Clankers sends daemon protocol frames, renders TUI state, manages attach-mode overlays, or maintains interactive-mode UI state
- **THEN** those behaviors remain in transport and UI adapters
- **THEN** the engine boundary stays free of protocol framing, TUI widget, and terminal event-loop types

### Requirement: Embedding-focused migration rails MUST verify the target architecture

The system MUST add explicit verification rails that keep future extraction work aligned with the embeddable engine target.

#### Scenario: engine API rails reject app-protocol leakage

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it checks the public `clankers-engine` surface for leakage of app-specific protocol or UI types such as `DaemonEvent`, `SessionCommand`, or TUI widget/runtime types
- **THEN** failure blocks acceptance of the capability work

#### Scenario: engine parity rails cover host-adapter seams

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it includes parity tests proving that controller and agent adapters execute engine-requested model/tool work and feed correlated host results back without re-deriving reusable turn policy locally
- **THEN** failure blocks acceptance of the capability work

#### Scenario: engine turn-state-machine rails cover positive and negative paths

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it includes positive and negative tests for prompt submission, model completion, tool-request planning, tool-result ingestion, retry decisions, cancellation, token-limit handling, and terminal stop behavior
- **THEN** the tests assert deterministic state/effect outcomes for the migrated reusable turn-orchestration slice

### Requirement: The first executable engine slice MUST keep authoritative turn state across model and tool boundaries

The system MUST keep `clankers-engine` authoritative for pending model/tool turn state after adapter code submits an accepted prompt.
r[embeddable-agent-engine.first-slice-authoritative-after-acceptance]

#### Scenario: model completion schedules engine-owned tool work
r[embeddable-agent-engine.model-completion-tool-state-retained]
- **WHEN** the host returns model output that requests one or more tools after accepted prompt submission
- **THEN** the engine updates its authoritative phase and pending tool-call state from that feedback
- **THEN** the returned effects enumerate the tool calls the host must execute without agent-local continuation branching deciding that tool plan independently

#### Scenario: accepted prompt submission creates engine-owned pending model work
r[embeddable-agent-engine.accepted-prompt-pending-model-work]

- **WHEN** a host adapter submits an accepted prompt into the migrated engine slice
- **THEN** the engine records engine-owned turn state and a correlated pending model request in `EngineState`
- **THEN** the returned effects include the model request the host must execute rather than requiring the runtime shell to reconstruct request state locally
- **THEN** the core lifecycle effect ID remains core-owned, adapter-held correlation data for later core completion feedback, not an `EngineState` field

#### Scenario: stop reasons and continuation decisions are engine-owned
r[embeddable-agent-engine.stop-continuation-policy-retained]

- **WHEN** a model completion stops normally, requests tools, hits a token limit, or fails after accepted prompt submission
- **THEN** the engine owns the policy for whether the turn stops, retries, schedules tool execution, or emits a failure outcome
- **THEN** app shells do not keep a second authoritative copy of those continuation rules

#### Scenario: tool-result ingestion is engine-owned
r[embeddable-agent-engine.tool-result-ingestion-retained]

- **WHEN** the host reports one or more tool results back to the engine
- **THEN** the engine decides how those results update conversation state and whether another model request is needed
- **THEN** app shells do not re-derive tool continuation policy locally

### Requirement: The migrated engine slice MUST own cancellation and correlation validation
The system MUST route cancellation and feedback validation for the migrated slice through engine-owned state, correlation IDs, and explicit rejections.

#### Scenario: mismatched feedback is rejected without state mutation
- **WHEN** the host reports model or tool feedback whose correlation ID does not match pending engine-owned work for the current state
- **THEN** the engine returns an explicit rejection
- **THEN** the previously valid state remains unchanged

#### Scenario: wrong-phase feedback is rejected without state mutation
- **WHEN** the host reports model completion, tool feedback, or cancellation in a phase where that input is not valid
- **THEN** the engine returns an explicit rejection describing the phase mismatch
- **THEN** the previously valid state remains unchanged

#### Scenario: cancellation clears pending work through engine-owned terminalization
- **WHEN** the host cancels a turn while model or tool work is pending in the migrated slice
- **THEN** the engine clears the pending engine-owned work for that slice and emits the terminal cancellation outcome/events for the host to surface
- **THEN** shells do not synthesize cancellation completion or terminal state outside the engine boundary

### Requirement: Agent and controller shells MUST adapt the migrated slice through engine-native types
The system MUST carry the migrated turn slice through `clankers-engine` state, inputs, effects, and correlated feedback instead of shell-local request-state tuples or duplicated continuation logic.

#### Scenario: agent runtime executes only engine-requested model and tool work
- **WHEN** `clankers-agent` runs the migrated turn slice
- **THEN** it executes provider and tool I/O only in response to `EngineEffect` values produced by the engine
- **THEN** it feeds the resulting success or failure payloads back through the matching `EngineInput` values rather than deciding next-step policy locally

#### Scenario: controller-facing seams stay adapter-only for the migrated slice
- **WHEN** controller-owned seams participate in the migrated slice
- **THEN** they translate shell-native state and events to or from engine-native values
- **THEN** they do not remain the authoritative owner of the migrated prompt/model/tool continuation policy

### Requirement: Verification rails MUST cover the executable engine slice
The system MUST verify the first executable engine slice with deterministic engine tests and adapter-parity rails.

#### Scenario: engine tests cover positive and negative first-slice paths
- **WHEN** validation runs for this migrated slice
- **THEN** engine-focused tests cover prompt submission, model completion with tool planning, tool-result continuation, tool failure, cancellation, and terminal finish outcomes
- **THEN** the same test suite covers negative paths for mismatched correlation IDs and wrong-phase feedback rejection

#### Scenario: adapter rails reject reintroduced shell-owned continuation logic
- **WHEN** validation runs for this migrated slice
- **THEN** parity or FCIS-style rails prove `clankers-agent::turn` and nearby adapters interpret engine effects and correlated inputs for the migrated slice
- **THEN** failure blocks acceptance if runtime shells reintroduce authoritative prompt/model/tool continuation policy outside `clankers-engine`

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

### Requirement: Engine contract dependencies MUST remain embeddable

The engine contract surface MUST depend only on reusable plain-data crates and MUST NOT require provider, router, daemon, UI, network, database, or async-runtime implementation crates to compile.
r[embeddable-agent-engine.minimal-contract-dependencies]

#### Scenario: engine cargo tree excludes runtime provider graph
r[embeddable-agent-engine.engine-cargo-tree-clean]
- **WHEN** validation inspects normal dependencies for `clankers-engine`
- **THEN** the dependency graph does not include `clankers-provider`, `clanker-router`, `tokio`, `reqwest`, `redb`, `iroh`, `ratatui`, `crossterm`, `portable-pty`, or `clankers-agent`
- **THEN** failure blocks acceptance of this change

#### Scenario: message contracts do not depend on router runtime
r[embeddable-agent-engine.message-without-router]
- **WHEN** validation inspects normal dependencies for `clanker-message`
- **THEN** the dependency graph does not include `clanker-router`, `clankers-provider`, `tokio`, `reqwest`, `reqwest-eventsource`, `redb`, `fs4`, `iroh`, `axum`, `tower-http`, `ratatui`, `crossterm`, or `portable-pty`
- **THEN** generic message, content, tool, thinking, usage, and stream contract types remain available from `clanker-message`

#### Scenario: router and provider consume canonical message contracts
r[embeddable-agent-engine.router-provider-reexports]
- **WHEN** router or provider code exposes LLM contract types used by existing Clankers call sites
- **THEN** those types are imported from or re-exported from the canonical `clanker-message` definitions
- **THEN** no independent duplicate `Usage`, `ToolDefinition`, `ThinkingConfig`, `MessageMetadata`, `ContentDelta`, `StreamDelta`, or other stream metadata/delta type identity is introduced
- **THEN** compile-time or static assertion tests prove the preserved router/provider compatibility paths resolve to the canonical `clanker-message` Rust type identities
- **THEN** representative serde JSON for moved usage, tool, thinking, message metadata, stream delta, and completion/provider request shapes remains compatible with the pre-migration router/provider shapes

### Requirement: Engine prompt submission MUST use engine-native transcripts

The engine prompt submission API MUST accept engine-native transcript data rather than Clankers shell message enums.
r[embeddable-agent-engine.engine-native-submission]

#### Scenario: engine no longer filters shell message variants
r[embeddable-agent-engine.no-agent-message-filtering]
- **WHEN** a host submits conversation context to the engine
- **THEN** the submitted messages are already canonical `EngineMessage` values
- **THEN** the engine does not depend on `AgentMessage` or decide how to drop Clankers-specific `BashExecution`, `Custom`, `BranchSummary`, or `CompactionSummary` messages

#### Scenario: Clankers adapter owns transcript conversion
r[embeddable-agent-engine.adapter-transcript-conversion]
- **WHEN** the Clankers agent runtime invokes the engine with its persisted conversation history
- **THEN** adapter code converts shell-native `AgentMessage` values into `EngineMessage` values before calling the engine
- **THEN** positive and negative tests cover included user/assistant/tool messages and excluded shell-only message variants

### Requirement: Boundary rails MUST prevent contract dependency regressions

The repository MUST provide deterministic validation rails that fail if the embeddable engine contract regains runtime or shell-only dependencies.
r[embeddable-agent-engine.contract-boundary-rails]

#### Scenario: cargo-tree rail rejects forbidden transitive crates
r[embeddable-agent-engine.cargo-tree-rail]
- **WHEN** the embeddable-engine validation bundle runs
- **THEN** it checks `cargo tree` output for `clankers-engine` and `clanker-message`
- **THEN** forbidden provider/router/runtime crates cause a clear failure message

#### Scenario: source rail rejects forbidden public surface imports
r[embeddable-agent-engine.source-surface-rail]
- **WHEN** the FCIS-style boundary test inventories non-test engine and message contract source
- **THEN** it fails on provider-shaped `CompletionRequest`, daemon protocol types, TUI types, Tokio handles, timestamps, shell-generated message IDs, shell request construction, or any non-test `AgentMessage` dependency/import/use inside `clankers-engine`
- **THEN** it allows adapter-only conversion code outside `clankers-engine`

### Requirement: Core and engine reducers MUST have explicit ownership boundaries

The system MUST keep `clankers-core` and `clankers-engine` as independently understandable reducers with explicit adapter composition rather than implicit state pass-through.
r[embeddable-agent-engine.core-engine-ownership]

#### Scenario: core-owned lifecycle policy stays in clankers-core
r[embeddable-agent-engine.core-owned-policy]
- **WHEN** behavior concerns prompt lifecycle, queued prompt replay, loop follow-up dispatch/completion, auto-test follow-up dispatch/completion, cancellation before engine submission, thinking-level changes, or disabled-tool filter state
- **THEN** the authoritative deterministic policy lives in `clankers-core`
- **THEN** controller adapters execute core effects and translate them into shell-native work for `clankers-agent` when needed, rather than duplicating that policy locally or moving it into the engine turn reducer

#### Scenario: engine-owned turn policy stays in clankers-engine
r[embeddable-agent-engine.engine-owned-policy]
- **WHEN** behavior concerns model request correlation, model completion, tool-call planning, tool feedback ingestion, retry scheduling, continuation budget, cancellation after an accepted prompt reaches model/tool/retry phases, or terminal turn outcomes
- **THEN** the authoritative deterministic policy lives in `clankers-engine`
- **THEN** controller and agent shells execute engine effects rather than duplicating that policy locally or moving it into `clankers-core`

### Requirement: Engine state MUST NOT carry dormant core state

The engine state MUST NOT include `CoreState` or other core reducer state as an unused pass-through field.
r[embeddable-agent-engine.no-dormant-core-state]

#### Scenario: engine state contains only active turn data
r[embeddable-agent-engine.engine-state-active-data]
- **WHEN** validation inspects `EngineState`
- **THEN** every field is owned by or actively used by the engine turn reducer
- **THEN** validation keeps an `EngineState` field inventory and one explicit reducer test or written justification per remaining field
- **THEN** no `CoreState`, `core_state`, or other `clankers-core` state field exists in `EngineState` for this change
- **THEN** any future active core/engine state composition requires a separate explicit no-std-core migration contract rather than an implicit engine field

#### Scenario: adapter composition is explicit
r[embeddable-agent-engine.explicit-adapter-composition]
- **WHEN** Clankers needs to combine prompt lifecycle policy with turn execution policy
- **THEN** controller-owned adapter code in `crates/clankers-controller/src/core_effects.rs` normalizes core prompt/follow-up effects
- **THEN** controller-owned adapter code in `crates/clankers-controller/src/core_engine_composition.rs` builds engine prompt submission plans from normalized core data
- **THEN** the composition seam is `pub(crate)`, adapter-owned, and testable without daemon protocol, TUI rendering, provider I/O, or tool execution

#### Scenario: adapter-held prompt correlation replaces engine core state
r[embeddable-agent-engine.adapter-held-prompt-correlation]
- **WHEN** controller adapters normalize an accepted core prompt or follow-up effect into an engine submission plan
- **THEN** the plan carries the originating `CoreEffectId`, accepted prompt kind, and `EngineInput::SubmitUserPrompt` outside `EngineState`
- **THEN** engine terminal success, failure, or cancellation is mapped by the adapter back into `CoreInput::PromptCompleted` for user prompts or `CoreInput::LoopFollowUpCompleted` for loop/auto-test follow-ups using the retained effect ID
- **THEN** mismatched or stale core completion feedback is rejected by `clankers-core`, not by `clankers-engine`

#### Scenario: cancellation phase ownership is explicit
r[embeddable-agent-engine.cancellation-phase-ownership]
- **WHEN** cancellation happens before a core-accepted prompt is submitted to the engine
- **THEN** the adapter reports cancelled lifecycle feedback to `clankers-core` and no engine input is created
- **WHEN** cancellation happens after the engine accepts a prompt and model/tool/retry work is pending
- **THEN** the `clankers-agent` turn adapter in `crates/clankers-agent/src/turn/{mod.rs,execution.rs}` sends `EngineInput::CancelTurn` to `clankers-engine`
- **WHEN** cancellation happens while retry-ready feedback is pending
- **THEN** `clankers-engine` owns the retry-wait cancellation outcome
- **WHEN** cancellation or feedback arrives after engine terminal output
- **THEN** it is handled as wrong-phase/post-terminal feedback without shell-local terminalization policy

### Requirement: Boundary rails MUST enforce reducer ownership

The repository MUST provide validation rails that catch core/engine ownership drift.
r[embeddable-agent-engine.core-engine-boundary-rails]

#### Scenario: engine dependency graph excludes clankers-core
r[embeddable-agent-engine.engine-excludes-core-dependency]
- **WHEN** validation inspects normal dependencies for `clankers-engine`
- **THEN** the dependency graph does not include `clankers-core`
- **THEN** core/engine composition happens in adapter code rather than through a direct engine-to-core crate dependency

#### Scenario: source rails reject cross-reducer policy leakage
r[embeddable-agent-engine.cross-reducer-source-rail]
- **WHEN** validation inventories non-test `crates/clankers-core/src/**`, `crates/clankers-engine/src/**`, all `crates/clankers-controller/src/**` with explicit allowlists for `core_effects.rs` and `core_engine_composition.rs`, `src/modes/event_loop_runner/**`, and `crates/clankers-agent/src/**`
- **THEN** it fails if the closed forbidden core-owned token inventory `core_state`, `clankers_core`, `CoreState`, `CoreInput`, `CoreEffect`, `CoreLogicalEvent`, `CoreEffectId`, `PromptRequested`, `PromptCompleted`, `ReplayQueuedPrompt`, `RunLoopFollowUp`, `LoopFollowUpCompleted`, `FollowUpDispatchAcknowledged`, `AutoTest`, `auto_test_enabled`, `auto_test_command`, `auto_test_in_progress`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, and `ToolFilterApplied`, `CompletionStatus`, `CoreFailure`, and `Cancelled` appear in non-test `clankers-engine` source
- **THEN** it fails if the closed forbidden engine-owned token inventory `clankers_engine`, `EngineState`, `EngineInput`, `EngineEffect`, `EngineModelRequest`, `EngineModelResponse`, `EngineToolRequest`, `EngineToolResult`, `EnginePromptSubmission`, `EngineCorrelationId`, `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, `ScheduleRetry`, `CancelTurn`, `EngineTerminalFailure`, `EngineEvent::TurnFinished`, `StopReason`, `terminal_failure`, `terminal_failure_outcome`, `retry_delay_for_attempt`, `retry_attempt`, `retry_budget`, `backoff`, `pending_model_request`, `pending_tool_calls`, `model_request_slot_budget`, `continuation_budget`, `ToolUse`, and `MaxTokens` appear in non-test `clankers-core` source
- **THEN** it fails if direct `CoreEffect::*` interpretation appears outside `crates/clankers-controller/src/core_effects.rs`
- **THEN** it fails if `EngineInput::*` glob imports appear in any non-test source
- **THEN** it fails if qualified or unqualified `SubmitUserPrompt` construction appears outside `crates/clankers-controller/src/core_engine_composition.rs`
- **THEN** it fails if qualified or unqualified `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, or `CancelTurn` construction appears outside `crates/clankers-agent/src/turn/{mod.rs,execution.rs}`
- **THEN** it fails if `RETRY_BACKOFF_BASE_SECONDS`, `RETRY_BACKOFF_EXPONENT_STEP`, `DEFAULT_MAX_MODEL_REQUESTS_PER_TURN`, `terminal_failure_outcome`, or `terminal_state_with_messages` appears outside `clankers-engine` source
- **THEN** it fails on core-owned symbols in `clankers-engine` such as `CoreState`, `CoreInput`, `CoreEffect`, prompt lifecycle, queued prompt, loop/auto-test follow-up, thinking, and disabled-tool variants
- **THEN** it fails on engine-owned symbols in `clankers-core` such as `EngineState`, `EngineInput`, `EngineEffect`, model/tool feedback, retry, post-submission cancellation, terminal, and continuation-budget variants, while allowing core-owned pre-submission cancellation feedback
- **THEN** it fails on controller/agent shell source outside named adapter seams that contains the listed qualified paths, unqualified variant constructors, glob imports, retry/backoff constants, or terminalization helper names

#### Scenario: agent public APIs stay shell-native for core lifecycle
r[embeddable-agent-engine.agent-core-type-rail]
- **WHEN** validation inventories non-test `crates/clankers-agent/src/**`
- **THEN** it fails if `clankers_core`, `CoreInput`, `CoreEffect`, `CoreState`, `CoreOutcome`, `CoreLogicalEvent`, `CoreEffectId`, `PromptCompleted`, `PostPromptEvaluation`, `FollowUpDispatchAcknowledged`, `LoopFollowUpCompleted`, `ToolFilterApplied`, `CompletionStatus`, `CoreFailure`, `Cancelled`, or `PostPromptAction` appear outside test-only code
- **THEN** controller-owned adapters remain the only core-type boundary for the migrated lifecycle slice

#### Scenario: composition tests cover positive and negative sequencing
r[embeddable-agent-engine.composition-tests]
- **WHEN** validation runs adapter composition tests
- **THEN** positive tests cover core prompt acceptance/start, adapter engine submission, engine turn execution, core completion feedback, and post-prompt follow-up evaluation in that order while preserving shell-visible prompt lifecycle, loop, auto-test, thinking, disabled-tool, retry, cancellation, and terminal behavior
- **THEN** negative tests cover out-of-order completion, mismatched effect IDs, wrong-phase engine feedback, and attempted lifecycle/turn feedback to the wrong reducer

