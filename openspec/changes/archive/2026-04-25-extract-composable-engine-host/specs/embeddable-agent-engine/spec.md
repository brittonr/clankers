## ADDED Requirements

### Requirement: Engine host MUST expose composable async execution contracts

The system MUST provide a reusable host-facing layer that interprets engine effects through caller-supplied model, tool, sleep, event, and cancellation adapters instead of requiring embedders to depend on `clankers-agent`.
r[embeddable-agent-engine.composable-host-contract]

#### Scenario: host runner drives engine effects through traits
r[embeddable-agent-engine.host-runner-traits]
- **WHEN** an embedding crate wants to run a complete engine turn
- **THEN** it can provide trait implementations for model execution, tool execution, retry sleeping, event emission, cancellation, and usage observation
- **THEN** the reusable runner executes `EngineEffect` values and feeds correlated `EngineInput` feedback back into the reducer
- **THEN** the runner does not require daemon, TUI, built-in tool bundle, session DB, provider/router implementation, network, async-runtime implementation, timestamp, shell-generated message ID, or Clankers prompt assembly dependencies
- **THEN** the runner does not own retry/backoff policy, retry budget state, continuation-budget decisions, token-limit terminalization, terminal stop policy, or tool-continuation policy; those decisions remain represented by `clankers-engine` state/effects
- **THEN** usage observers receive usage deltas in stream arrival order plus one final usage summary after model completion, and usage observation failures are reported as adapter diagnostics without changing reducer feedback or terminal behavior

#### Scenario: Clankers agent becomes default assembly
r[embeddable-agent-engine.agent-default-assembly]
- **WHEN** existing Clankers interactive, daemon, or attach flows run a turn
- **THEN** they use the reusable host runner through Clankers-specific adapters
- **THEN** existing shell-visible behavior for streaming, tool execution, retries, cancellation, usage updates, model switching, hooks, event emission, and event ordering remains unchanged
- **THEN** the existing `clankers-agent::Agent` public assembly/API remains available as the default Clankers wiring over the reusable host pieces

### Requirement: Tool execution surface MUST be reusable outside clankers-agent

The system MUST provide a tool-host surface that can execute tool calls requested by the engine without importing the full Clankers agent runtime.
r[embeddable-agent-engine.reusable-tool-host]

#### Scenario: tool catalog and executor are independent host components
r[embeddable-agent-engine.tool-host-catalog]
- **WHEN** an embedding crate builds an agent with a custom tool set
- **THEN** it can supply a tool catalog and executor compatible with engine tool-call effects
- **THEN** the executor supports successful results, tool errors, missing tools, capability denial, cancellation, output truncation, and reusable result accumulation as explicit host outcomes
- **THEN** generic hook seams run around tool execution without requiring the generic tool-host crate to depend on Clankers hook pipeline types

#### Scenario: tool-host outcomes and usage failures are verified
r[embeddable-agent-engine.tool-host-outcome-verification]
- **WHEN** validation runs for reusable host/tool-host behavior
- **THEN** tests cover missing tools, capability denial, tool cancellation, output truncation, result accumulation, and hook ordering
- **THEN** tests cover usage-observer failure being recorded as adapter diagnostics without changing reducer feedback or terminal behavior

#### Scenario: plugin-backed tools share the same executor seam
r[embeddable-agent-engine.plugin-tool-adapter]
- **WHEN** WASM or stdio plugin tools are enabled
- **THEN** they are exposed through the same tool-host executor seam as built-in tools
- **THEN** plugin runtime details remain outside `clankers-engine` and outside the generic host-runner policy

### Requirement: Stream accumulation MUST be reusable deterministic logic

The system MUST expose deterministic stream-folding logic in the `clankers-engine-host` stream module that turns provider stream events into model responses without depending on Clankers TUI or event bus plumbing.
r[embeddable-agent-engine.reusable-stream-accumulator]

#### Scenario: stream folding handles normal model output
r[embeddable-agent-engine.stream-folding-positive]
- **WHEN** a model stream emits text, thinking, tool-use JSON deltas, usage deltas, and message stop events
- **THEN** reusable accumulator logic returns canonical assistant content, usage, model name, and stop reason
- **THEN** UI/event-bus forwarding remains adapter-only behavior around that deterministic fold
- **THEN** `clankers-engine-host` stream accumulation does not depend on daemon protocol, TUI crates, session DB, built-in tool bundles, plugin runtime supervision, or Clankers-specific provider discovery

#### Scenario: stream folding rejects or normalizes malformed inputs deterministically
r[embeddable-agent-engine.stream-folding-negative]
- **WHEN** a model stream emits malformed tool JSON, non-object tool JSON, missing block starts, duplicate indexes, late deltas, or provider error events
- **THEN** malformed tool JSON returns explicit `MalformedToolJson` with the block index
- **THEN** non-object tool JSON returns explicit `NonObjectToolJson` with the block index
- **THEN** deltas before block start return explicit `MissingContentBlockStart`
- **THEN** duplicate block starts return explicit `DuplicateContentBlockIndex`
- **THEN** deltas after block stop return explicit `LateContentDelta`
- **THEN** provider error events return explicit provider error results preserving status and retryability
- **THEN** usage-only deltas normalize as usage observations without assistant content
- **THEN** message stop without content normalizes as an empty assistant response with stop reason
- **THEN** positive and negative accumulator tests cover those paths without standing up a provider or TUI
- **THEN** at least one parser/adapter seam test feeds raw provider stream bytes or events through the real stream-normalization entrypoint before the accumulator result is asserted

### Requirement: Host extraction rails MUST prevent clankers-agent from regaining runner ownership

The system MUST add validation rails proving reusable async turn-driving policy lives in the host layer and `clankers-agent` remains the Clankers default assembly.
r[embeddable-agent-engine.host-extraction-rails]

#### Scenario: source rails reject duplicated runner policy
r[embeddable-agent-engine.no-duplicated-runner-policy]
- **WHEN** validation inventories non-test `clankers-agent::turn` code after extraction
- **THEN** it fails if that code reintroduces authoritative model/tool/retry/cancellation continuation loops instead of delegating to the reusable host runner
- **THEN** adapter code may still translate Clankers events, build provider requests, emit hooks, update usage, and bridge model-switch state

#### Scenario: host crates stay dependency-light
r[embeddable-agent-engine.host-crate-boundary-rails]
- **WHEN** validation inspects `clankers-engine-host` and `clankers-tool-host` normal dependency graphs
- **THEN** `clankers-engine-host` does not depend on daemon protocol, TUI crates, session DB, built-in tool bundles, plugin runtime supervision, system-prompt assembly, `clankers-provider`, `clanker-router`, provider-shaped `CompletionRequest`, network crates, async-runtime implementation crates, timestamps, shell-generated message IDs, or provider discovery crates
- **THEN** `clankers-tool-host` does not depend on daemon protocol, TUI crates, session DB, model-selection policy, system-prompt assembly, built-in tool bundles, plugin runtime supervision, `clankers-provider`, `clanker-router`, provider-shaped `CompletionRequest`, network crates, async-runtime implementation crates, timestamps, shell-generated message IDs, Clankers-specific provider discovery, or engine reducer internals beyond `clankers-engine::EngineToolCall`, `EngineCorrelationId`, `Content`, and plain tool request/result content structs owned by `clankers-tool-host`; legacy names `EngineToolRequest`/`EngineToolResult` remain forbidden reducer-internal/source-rail tokens, while the current reusable engine tool-call payload is `EngineToolCall`
- **THEN** source rails fail if the host runner reintroduces retry/backoff constants, continuation-budget policy, token-limit terminalization, terminalization helper names, or tool-continuation decisions outside `clankers-engine`
- **THEN** source rails fail if `clankers-engine-host` public contracts expose provider-shaped request/response types, shell-native `AgentMessage`, timestamps, shell-generated message IDs, or network/runtime handles

#### Scenario: runtime parity rails cover host adapters
r[embeddable-agent-engine.host-adapter-parity]
- **WHEN** validation runs after host extraction
- **THEN** focused runtime tests prove the Clankers adapters preserve streaming deltas, tool-call events, tool failures, retry backoff behavior, cancellation behavior, usage updates, hook dispatch, model switching, and relative event ordering while using the reusable host runner

### Requirement: Workspace generated artifacts MUST include extracted host crates

When host crates are introduced, workspace metadata and generated artifacts MUST be refreshed so Cargo, Nix, and docs see the same crate graph.
r[embeddable-agent-engine.host-artifact-freshness]

#### Scenario: host crate artifact refresh is validated
r[embeddable-agent-engine.host-artifact-refresh]
- **WHEN** `clankers-engine-host` and `clankers-tool-host` are added to the workspace
- **THEN** validation evidence includes updated workspace manifests, `Cargo.lock`, `flake.nix` test/check crate lists, `build-plan.json` generated by `unit2nix --workspace --force --no-check -o build-plan.json`, and generated docs from `cargo xtask docs`
- **THEN** the artifact checks fail if either new host crate is missing from the generated workspace artifacts

## MODIFIED Requirements

### Requirement: Boundary rails MUST enforce reducer ownership

The repository MUST provide validation rails that catch core/engine ownership drift while allowing the reusable host runner to own effect-feedback construction after extraction.
r[embeddable-agent-engine.core-engine-boundary-rails]

#### Scenario: engine dependency graph excludes clankers-core
r[embeddable-agent-engine.engine-excludes-core-dependency]
- **WHEN** validation inspects normal dependencies for `clankers-engine`
- **THEN** the dependency graph does not include `clankers-core`
- **THEN** core/engine composition happens in adapter code rather than through a direct engine-to-core crate dependency

#### Scenario: engine feedback construction moves to the host runner seam
r[embeddable-agent-engine.host-feedback-construction-seam]
- **WHEN** validation inventories non-test engine feedback constructors after host extraction
- **THEN** `EngineInput::SubmitUserPrompt` construction remains controller composition owned in `crates/clankers-controller/src/core_engine_composition.rs`
- **THEN** correlated `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, and `EngineInput::CancelTurn` construction is allowed only in `crates/clankers-engine-host/src/runner.rs`, `crates/clankers-engine-host/src/runtime/**`, and test-only code and removed from `clankers-agent::turn` except adapter tests
- **THEN** `clankers-agent` may call the host runner and translate Clankers shell data, but it must not regain the authoritative model/tool/retry/cancel feedback loop

#### Scenario: cross-reducer source rail allows the host runner feedback seam
r[embeddable-agent-engine.cross-reducer-source-rail]
- **WHEN** validation inventories engine feedback constructors after host extraction
- **THEN** all existing `r[embeddable-agent-engine.cross-reducer-source-rail]` inventories over `crates/clankers-core/src/**`, `crates/clankers-engine/src/**`, controller allowlists, `src/modes/event_loop_runner/**`, and `crates/clankers-agent/src/**` remain enforced
- **THEN** the existing forbidden core-owned and engine-owned token inventories, `CoreEffect::*` interpretation ownership, `EngineInput::*` glob import rejection, retry/backoff constant checks, and terminalization helper checks remain enforced
- **THEN** only the feedback-construction allowlist changes: `EngineInput::SubmitUserPrompt` construction remains allowed only in controller composition code, while correlated `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, and `CancelTurn` feedback construction is allowed only in `crates/clankers-engine-host/src/runner.rs`, `crates/clankers-engine-host/src/runtime/**`, and test-only code
- **THEN** non-test `clankers-agent::turn` must delegate to `clankers-engine-host` and must not construct those feedback variants directly

#### Scenario: agent public APIs stay shell-native for core lifecycle
r[embeddable-agent-engine.agent-core-type-rail]
- **WHEN** validation inventories non-test `crates/clankers-agent/src/**`
- **THEN** it fails if `clankers_core`, `CoreInput`, `CoreEffect`, `CoreState`, `CoreOutcome`, `CoreLogicalEvent`, `CoreEffectId`, `PromptCompleted`, `PostPromptEvaluation`, `FollowUpDispatchAcknowledged`, `LoopFollowUpCompleted`, `ToolFilterApplied`, `CompletionStatus`, `CoreFailure`, `Cancelled`, or `PostPromptAction` appear in non-test `crates/clankers-agent/src/**` outside explicitly allowed controller/adapter seams
- **THEN** controller-owned adapters remain the only core-type boundary for the migrated lifecycle slice

#### Scenario: composition tests cover positive and negative sequencing
r[embeddable-agent-engine.composition-tests]
- **WHEN** validation runs adapter composition tests
- **THEN** positive tests cover core prompt acceptance/start, adapter engine submission, host-runner engine turn execution, core completion feedback, and post-prompt follow-up evaluation in that order while preserving shell-visible prompt lifecycle, loop, auto-test, thinking, disabled-tool, retry, cancellation, and terminal behavior
- **THEN** negative tests cover out-of-order completion, mismatched effect IDs, wrong-phase engine feedback, and attempted lifecycle/turn feedback to the wrong reducer

### Requirement: Engine state MUST NOT carry dormant core state

The engine state MUST NOT include `CoreState` or other core reducer state as an unused pass-through field, and host extraction MUST keep cancellation ownership explicit.
r[embeddable-agent-engine.no-dormant-core-state]

#### Scenario: engine state contains only active turn data
r[embeddable-agent-engine.engine-state-active-data]
- **WHEN** validation inspects `EngineState`
- **THEN** every `EngineState` field is owned by or actively used by the engine turn reducer
- **THEN** validation keeps an `EngineState` field inventory and one explicit reducer test or written justification per remaining field
- **THEN** no `CoreState`, `core_state`, or other `clankers-core` state field exists in `EngineState` for this change
- **THEN** any future active core/engine state composition requires a separate explicit no-std-core migration contract rather than an implicit engine field

#### Scenario: adapter composition is explicit
r[embeddable-agent-engine.explicit-adapter-composition]
- **WHEN** Clankers combines prompt lifecycle policy with turn execution policy
- **THEN** controller-owned adapter code in `crates/clankers-controller/src/core_effects.rs` normalizes core prompt/follow-up effects
- **THEN** controller-owned adapter code in `crates/clankers-controller/src/core_engine_composition.rs` builds engine prompt submission plans from normalized core data
- **THEN** the composition seam is `pub(crate)`, adapter-owned, and testable without daemon protocol, TUI rendering, provider I/O, or tool execution

#### Scenario: adapter-held prompt correlation replaces engine core state
r[embeddable-agent-engine.adapter-held-prompt-correlation]
- **WHEN** controller adapters normalize an accepted core prompt or follow-up effect into an engine submission plan
- **THEN** the plan carries the originating `CoreEffectId`, accepted prompt kind, and engine prompt seed outside `EngineState`
- **THEN** engine terminal success, failure, or cancellation is mapped by the adapter back into `CoreInput::PromptCompleted` for user prompts or `CoreInput::LoopFollowUpCompleted` for loop/auto-test follow-ups using the retained effect ID
- **THEN** mismatched or stale core completion feedback is rejected by `clankers-core`, not by `clankers-engine` or `clankers-engine-host`

#### Scenario: cancellation phase ownership is explicit
r[embeddable-agent-engine.cancellation-phase-ownership]
- **WHEN** cancellation happens before a core-accepted prompt is submitted to the engine
- **THEN** the adapter reports cancelled lifecycle feedback to `clankers-core` and no engine input is created
- **WHEN** cancellation happens after the engine accepts a prompt and model/tool/retry work is pending
- **THEN** `clankers-engine-host` converts the injected cancellation source into correlated `EngineInput::CancelTurn`
- **THEN** `clankers-agent::turn` supplies the Clankers cancellation adapter but does not construct `EngineInput::CancelTurn` directly
- **WHEN** cancellation happens while retry-ready feedback is pending
- **THEN** `clankers-engine` owns the retry-wait cancellation outcome
- **WHEN** cancellation or feedback arrives after engine terminal output
- **THEN** it is handled as wrong-phase/post-terminal feedback without shell-local terminalization policy

### Requirement: Adapter parity rails MUST cover retry, budget, and token-limit ownership

The repository MUST keep retry scheduling, terminal policy, continuation policy, and post-acceptance cancellation in `clankers-engine` plus the reusable host runner rather than in `clankers-agent::turn`.
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
- **WHEN** invalid retry/model feedback is sent to the reducer
- **THEN** model-success or model-failure feedback while the engine is waiting for retry-ready feedback is rejected until a matching retry-ready input is accepted
- **THEN** matching retry-ready input is valid only in the retry-waiting phase and re-emits the model request as specified by retry scheduling
- **THEN** the engine returns explicit rejection such as `EngineRejection::CorrelationMismatch` for wrong IDs or `EngineRejection::InvalidPhase` for wrong-phase and post-terminal feedback
- **THEN** the engine leaves state unchanged and emits no effects

#### Scenario: runtime adapter rails reject local policy re-derivation
r[embeddable-agent-engine.adapter-rail]
- **WHEN** validation runs after host adapter migration
- **THEN** non-test `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}` still fails on retry-budget or retry-backoff constants, arithmetic to choose retry delays, continuation-budget branching, and terminal policy branching
- **THEN** non-test `crates/clankers-agent/src/turn/{mod.rs,execution.rs}` must delegate to `clankers-engine-host` and must not match `EngineEffect::ScheduleRetry`, construct `RetryReady`, construct `CancelTurn`, branch on `StopReason::MaxTokens` or `StopReason::ToolUse` for engine terminal policy, or own the model/tool/retry/cancellation feedback loop
- **THEN** provider stop-string parsing into `StopReason`, provider request conversion in `turn/execution.rs`, and named adapter constants in `crates/clankers-agent/src/lib.rs` remain allowed only as shell adapter translation, not as reducer policy
- **THEN** focused runtime adapter tests prove shell-visible retry, cancellation, budget exhaustion, zero-budget rejection, token-limit terminalization, and terminal behavior remains unchanged while using engine-owned decisions through the host runner
