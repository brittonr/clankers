## MODIFIED Requirements

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

## ADDED Requirements

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
