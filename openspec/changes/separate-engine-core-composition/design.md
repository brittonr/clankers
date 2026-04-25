## Verification Summary

This change is complete when reducer ownership is visible in code and tests: `EngineState` no longer carries dormant `CoreState` pass-through data, adapter composition is explicit and pure, and source rails fail if lifecycle policy moves into the engine or turn policy moves back into core/controller/agent shells.

## Context

`clankers-core` owns no-std lifecycle and control policy. `clankers-engine` owns model/tool turn policy. The current engine state still carries `core_state: Option<CoreState>` even though the engine reducer only clones it through transitions. That dormant field blurs ownership and invites future policy drift.

## Goals / Non-Goals

**Goals**

- Remove dormant core state from `clankers-engine` unless it becomes actively reduced through a documented, tested composition contract.
- Keep `clankers-core` and `clankers-engine` as separate reducers with explicit adapter composition.
- Clarify which adapter module may interpret core effects and which module may build engine input from normalized adapter data.
- Add deterministic positive and negative composition tests.
- Extend FCIS/source rails for core/engine/controller/agent ownership drift.

**Non-Goals**

- Do not merge `clankers-core` and `clankers-engine`.
- Do not move provider I/O, tool I/O, daemon protocol, TUI rendering, hooks, Tokio coordination, or sleeping into either reducer or composition helper.
- Do not change user-visible lifecycle or turn behavior except for removing dormant pass-through state.
- Do not expose composition helpers as public crate API unless implementation proves a downstream need.

## Decisions

### 1. Reducers stay separate

`clankers-core` remains the no-std reducer for prompt lifecycle, queued prompt replay, loop/auto-test follow-ups, thinking, and disabled-tool filter state. `clankers-engine` remains the reducer for model/tool turn progression, retry, continuation budget, cancellation during turn phases, and terminal turn outcomes.

### 2. No dormant composition fields

`EngineState` must remove `core_state: Option<CoreState>`. `crates/clankers-engine/src/**` must not import or store `clankers_core` state. If a future change needs active core/engine state composition, it must add an explicit contract, reducer inputs/effects, tests, and rails.

### 3. Core effect interpretation stays centralized

`crates/clankers-controller/src/core_effects.rs` remains the single controller-owned interpreter for `clankers-core::CoreEffect` variants. It may normalize an accepted `CoreEffect::StartPrompt` / follow-up effect into plain adapter data, but other controller files must not match core effect variants directly.

### 4. Composition helper consumes normalized adapter data

`crates/clankers-controller/src/core_engine_composition.rs` is a new `pub(crate)` pure adapter module. It does not match `CoreEffect` variants. It consumes controller-normalized data produced by `core_effects.rs` and builds the initial engine-native prompt submission plan. It does not dispatch non-prompt engine feedback in production.

Initial local structs/functions:

```rust
pub(crate) enum AcceptedPromptKind {
    UserPrompt,
    FollowUp(clankers_core::FollowUpSource),
}

pub(crate) struct AcceptedPromptStart {
    pub core_effect_id: clankers_core::CoreEffectId,
    pub prompt_text: String,
    pub image_count: u32,
    pub kind: AcceptedPromptKind,
}

pub(crate) struct EngineSubmissionPolicy {
    pub max_tokens: Option<usize>,
    pub temperature: Option<f32>,
    pub thinking: Option<clanker_message::ThinkingConfig>,
    pub no_cache: bool,
    pub cache_ttl: Option<std::time::Duration>,
    pub session_id: String,
    pub model_request_slot_budget: usize,
}

#[cfg(test)]
pub(crate) enum CompositionReducer {
    CoreLifecycle,
    EngineTurn,
}

#[cfg(test)]
pub(crate) enum CompositionFeedback {
    Core(clankers_core::CoreInput),
    Engine(clankers_engine::EngineInput),
}

#[cfg(test)]
pub(crate) enum CompositionRejection {
    LifecycleFeedbackSentToEngine,
    TurnFeedbackSentToCore,
}

#[cfg(test)]
pub(crate) enum CompositionStep {
    Core(clankers_core::CoreOutcome),
    Engine(clankers_engine::EngineOutcome),
}

#[cfg(test)]
pub(crate) fn apply_composition_feedback_for_tests(
    target: CompositionReducer,
    core_state: &clankers_core::CoreState,
    engine_state: &clankers_engine::EngineState,
    feedback: CompositionFeedback,
) -> Result<CompositionStep, CompositionRejection>;

pub(crate) struct EngineSubmissionPlan {
    pub core_effect_id: clankers_core::CoreEffectId,
    pub prompt_kind: AcceptedPromptKind,
    pub engine_input: clankers_engine::EngineInput,
}

pub(crate) fn engine_submission_from_prompt_start(
    prompt_start: &AcceptedPromptStart,
    prior_messages: Vec<clankers_engine::EngineMessage>,
    model: String,
    tools: Vec<clanker_message::ToolDefinition>,
    system_prompt: String,
    request_policy: EngineSubmissionPolicy,
) -> EngineSubmissionPlan;
```

`EngineSubmissionPolicy` is plain data already needed by `EnginePromptSubmission`; it carries no provider object, model selector, hook pipeline, DB handle, daemon/TUI value, Tokio handle, channel, or sleep primitive.

### 5. Engine-prompt gates live in `core_effects.rs`

`core_effects.rs` owns the one `CoreEffect` normalization helper:

```rust
pub(crate) enum CoreEffectGateRejection {
    CoreRejected(clankers_core::CoreError),
    MissingEnginePromptEffect,
    ReplayQueuedPromptNeedsFreshCorePrompt,
    UnexpectedCoreEffect,
}

pub(crate) enum AcceptedEnginePrompt {
    UserPrompt(crate::core_engine_composition::AcceptedPromptStart),
    FollowUp(crate::core_engine_composition::AcceptedPromptStart),
}

pub(crate) fn accepted_engine_prompt_from_core_outcome(
    core_outcome: &clankers_core::CoreOutcome,
) -> Result<AcceptedEnginePrompt, CoreEffectGateRejection>;
```

`EngineSubmissionPlan.core_effect_id` and `AcceptedPromptKind` are retained by the controller prompt execution shell until completion. `AcceptedPromptKind::UserPrompt` completes through `CoreInput::PromptCompleted { effect_id }`. `AcceptedPromptKind::FollowUp(LoopContinuation | AutoTest)` first receives `CoreInput::FollowUpDispatchAcknowledged { effect_id, Accepted }` when dispatch is accepted and later completes through `CoreInput::LoopFollowUpCompleted { effect_id }`; the existing core reducer uses that input for both loop and auto-test follow-up sources. The same identity also remains present in core pending state while work is in flight. Adapters cannot build `EngineInput::SubmitUserPrompt` until core accepts an engine-submittable lifecycle transition and `core_effects.rs` normalizes it. `CoreEffect::StartPrompt` normalizes to `AcceptedEnginePrompt::UserPrompt` with `AcceptedPromptKind::UserPrompt`. `CoreEffect::RunLoopFollowUp { source: LoopContinuation | AutoTest, .. }` normalizes to `AcceptedEnginePrompt::FollowUp` with `AcceptedPromptKind::FollowUp(source)`. `CoreEffect::ReplayQueuedPrompt` intentionally does not create engine work; it returns `ReplayQueuedPromptNeedsFreshCorePrompt`, requiring the shell to submit the queued prompt back through `CoreInput::PromptRequested` first. Rejected core outcomes, missing prompt/follow-up effects, and unrelated lifecycle effects do not create engine work.

## Composition Tests

Unit tests live in two modules:

- `crates/clankers-controller/src/core_effects.rs` tests `accepted_engine_prompt_from_core_outcome` positive/negative `CoreEffect` normalization.
- `crates/clankers-controller/src/core_engine_composition.rs` tests reducer target dispatch and engine input construction from normalized adapter data.

Validation commands:

```bash
cargo test -p clankers-controller core_engine_composition
cargo test -p clankers-controller accepted_engine_prompt
cargo test -p clankers-agent engine_feedback
```

Required positive cases:

- prompt start: accepted `CoreInput::PromptRequested` yields normalized `AcceptedEnginePrompt::UserPrompt`, then `engine_submission_from_prompt_start` returns an `EngineSubmissionPlan` containing both the matching `core_effect_id` and `EngineInput::SubmitUserPrompt`.
- prompt completion: accepted `CoreInput::PromptCompleted` clears user-prompt busy state and permits later `CoreInput::EvaluatePostPrompt` to select queued prompt / loop / auto-test follow-up; accepted follow-up completions use `CoreInput::LoopFollowUpCompleted` for both loop and auto-test follow-up sources.
- engine turn execution: `EngineInput::SubmitUserPrompt` is built by `core_engine_composition.rs`; `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, and `CancelTurn` are covered by agent turn adapter tests and may appear in `core_engine_composition.rs` only inside `#[cfg(test)]` reducer-dispatch tests.
- terminal completion: engine terminal outcome does not mutate core lifecycle state without a later explicit core lifecycle input.
- queued prompt replay returns `ReplayQueuedPromptNeedsFreshCorePrompt`; loop follow-up and auto-test follow-up normalize from `CoreEffect::RunLoopFollowUp` before any engine prompt submission is built.

Required negative cases:

- prompt completion before a matching core pending prompt returns core out-of-order/mismatch rejection.
- mismatched core effect IDs return core mismatch rejections.
- wrong-phase engine feedback returns engine rejection without mutating engine state.
- post-terminal engine feedback returns engine rejection without mutating engine state.
- test-only `CompositionFeedback::Core` sent to `CompositionReducer::EngineTurn` returns `LifecycleFeedbackSentToEngine`.
- test-only `CompositionFeedback::Engine` sent to `CompositionReducer::CoreLifecycle` returns `TurnFeedbackSentToCore`.

## Source Rail Inventory

Persistent rail: `crates/clankers-controller/tests/fcis_shell_boundaries.rs`.

Validation command:

```bash
cargo test -p clankers-controller --test fcis_shell_boundaries
./scripts/check-llm-contract-boundary.sh
```

The rail uses exact path/segment inventories and prints the matched file plus symbol.

### Engine source checks

For every non-test file under `crates/clankers-engine/src/`, reject:

- dormant core-state symbols: `core_state`, `clankers_core`, `CoreState`, `CoreInput`, `CoreEffect`, `CoreLogicalEvent`, `PendingPromptState`, `PendingFollowUpState`, `PendingToolFilterState`
- core lifecycle/control symbols: `PromptRequested`, `PromptCompleted`, `ReplayQueuedPrompt`, `RunLoopFollowUp`, `LoopFollowUpCompleted`, `FollowUpDispatchAcknowledged`, `AutoTest`, `auto_test_enabled`, `auto_test_command`, `auto_test_in_progress`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, `ToolFilterApplied`

### Core source checks

For every non-test file under `crates/clankers-core/src/`, reject engine-owned turn symbols: `clankers_engine`, `EngineState`, `EngineInput`, `EngineEffect`, `EngineModelRequest`, `EngineModelResponse`, `EngineToolRequest`, `EngineToolResult`, `EnginePromptSubmission`, `EngineCorrelationId`, `RetryReady`, `ScheduleRetry`, `CancelTurn`, `EngineTerminalFailure`, `EngineEvent::TurnFinished`, `StopReason`, `terminal_failure`, `terminal_failure_outcome`, `retry_delay_for_attempt`, `retry_attempt`, `retry_attempts`, `retry_budget`, `backoff`, `pending_model_request`, `pending_tool_calls`, `model_request_budget`, `continuation_budget`, `tool_call`, `tool_calls`, `ToolUse`, `ModelCompleted`, `ModelFailed`, `ToolCompleted`, and `ToolFailed`.

### Controller source checks

- `core_effects.rs` is the only controller runtime file allowed to match any `CoreEffect::` variant. Every other non-test controller runtime file must reject the `CoreEffect::` path segment.
- `core_engine_composition.rs` is the only controller runtime file allowed to construct `EngineInput::SubmitUserPrompt`. Approved controller adapter files (`command.rs`, `core_effects.rs`, `auto_test.rs`) may call `engine_submission_from_prompt_start`, but must not construct `EngineInput::SubmitUserPrompt` directly.
- `core_engine_composition.rs` must reject imports/paths for daemon protocol, TUI, providers, tools, hooks, Tokio, channels, and sleeping: `clankers_protocol`, `DaemonEvent`, `SessionCommand`, `clanker_tui_types`, `ratatui`, `crossterm`, `clankers_provider`, `Provider`, `CompletionProvider`, `HookPipeline`, `ToolRegistry`, `ToolExecutor`, `ToolCallExecutor`, `tokio`, `mpsc`, `oneshot`, `JoinHandle`, `sleep`, and `Sleep`.

- All non-test controller runtime files, including `core_engine_composition.rs`, must not contain engine feedback/policy symbols: `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, `ScheduleRetry`, `CancelTurn`, `EngineTerminalFailure`, `EngineEvent::TurnFinished`, `StopReason::ToolUse`, `pending_model_request`, `pending_tool_calls`, `retry_attempt`, `retry_budget`, `terminal_failure`, or `terminal_failure_outcome`. `core_engine_composition.rs` is allowed only `EngineInput::SubmitUserPrompt` and request-planning fields.

### Embedded runtime source checks

Embedded runtime coverage is recursive over every non-test Rust file under `src/modes/event_loop_runner/**`. Non-test embedded runtime files must not match `CoreEffect::*`, construct `clankers_core::CoreInput` lifecycle feedback directly, or contain queued-prompt precedence / loop-follow-up / auto-test follow-up policy symbols outside controller-owned adapter calls. Exact forbidden path/segment inventory for those files includes `clankers_core`, `CoreInput::PromptCompleted`, `CoreInput::EvaluatePostPrompt`, `CoreInput::FollowUpDispatchAcknowledged`, `CoreInput::LoopFollowUpCompleted`, `CoreEffect::ReplayQueuedPrompt`, `CoreEffect::RunLoopFollowUp`, `PostPromptEvaluation`, `FollowUpDispatchAcknowledged`, and `LoopFollowUpCompleted`.

### Agent source checks

Existing coverage over `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}` remains active. Agent files may call `clankers_engine::reduce` and convert provider/tool payloads, but must not contain engine-owned policy helper names or constants such as `retry_delay_for_attempt`, `terminal_failure_outcome`, `terminal_state_with_messages`, `retry_attempts_remaining`, `DEFAULT_MAX_MODEL_REQUESTS_PER_TURN`, `RETRY_BACKOFF_BASE_SECONDS`, or `RETRY_BACKOFF_EXPONENT_STEP`.

## Risks / Trade-offs

**False-positive rails** are mitigated by explicit allowed files and exact forbidden symbols.

**Test churn from removing `core_state`** is expected; update constructors and tests without changing reducer behavior.

**Composition over-abstraction** is mitigated by keeping helpers `pub(crate)`, pure, and small. Shell I/O stays in existing adapters.

## Design Clarifications from Gate Review

- Agent core-type rail: non-test `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}` must continue rejecting direct prompt-lifecycle core types and paths: `clankers_core`, `CoreInput`, `CoreEffect`, `CoreState`, `CoreOutcome`, `CoreLogicalEvent`, `PromptCompleted`, `PostPromptEvaluation`, `FollowUpDispatchAcknowledged`, `LoopFollowUpCompleted`, and `ToolFilterApplied`. This preserves the existing shell-native agent API rule; core translation stays in controller-owned adapters.
- Engine feedback construction seam: model/tool engine feedback is constructed only in the agent turn adapter, not in controller runtime files. Allowed construction paths are `crates/clankers-agent/src/turn/{mod.rs,execution.rs}` for `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, and `EngineInput::CancelTurn`, and those files must feed them to `clankers_engine::reduce`. Controller files remain forbidden from those symbols except `core_engine_composition.rs` for `EngineInput::SubmitUserPrompt` planning from normalized core prompt data.
- Existing non-prompt `CoreEffect` interpretation is part of `core_effects.rs`, not new composition logic. Current helpers such as `execute_thinking_effects`, `execute_tool_filter_request_effects`, `execute_tool_filter_feedback_effects`, `execute_start_loop_effects`, `execute_stop_loop_effects`, `execute_post_prompt_effects`, and `execute_follow_up_dispatch_effects` remain the centralized interpreter functions. This change may refactor them for pure-core extraction, but it must not move their variant matching into `command.rs`, `auto_test.rs`, embedded runtime files, or agent adapters.


### Agent feedback rail details

The FCIS rail must make the agent feedback seam concrete:

- Allowed non-test `EngineInput` feedback constructors are limited to `crates/clankers-agent/src/turn/mod.rs` and `crates/clankers-agent/src/turn/execution.rs`.
- Other non-test files under `crates/clankers-agent/src/` must reject `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, and `EngineInput::CancelTurn`.
- Each allowed agent turn file that constructs any of those feedback variants must also contain a `clankers_engine::reduce` call or a call to a local helper whose only purpose is to call `clankers_engine::reduce`; otherwise the rail fails.
- Allowed agent turn files must still reject engine-owned policy helper names and constants: `retry_delay_for_attempt`, `terminal_failure_outcome`, `terminal_state_with_messages`, `retry_attempts_remaining`, `DEFAULT_MAX_MODEL_REQUESTS_PER_TURN`, `RETRY_BACKOFF_BASE_SECONDS`, and `RETRY_BACKOFF_EXPONENT_STEP`.


### Agent adapter test target

The agent turn adapter coverage referenced above is a new focused test filter `engine_feedback`. It must include positive tests proving model and tool feedback inputs constructed by `clankers-agent::turn` are fed into `clankers_engine::reduce`, and negative tests proving wrong-phase/post-terminal feedback remains an engine rejection rather than being terminalized by agent-local policy.


### Rail parser hardening

`fcis_shell_boundaries.rs` must continue using the existing non-test AST collectors that skip `#[cfg(test)]` modules, items, statements, fields, variants, and match arms before applying forbidden-symbol checks. Colocated unit tests may therefore use forbidden runtime symbols inside `#[cfg(test)]` code without failing the non-test source rail.

The rail must also catch unqualified imports and variant construction. For core-effect checks, reject `CoreEffect::*`, direct imports of `StartPrompt`, `ReplayQueuedPrompt`, `RunLoopFollowUp`, `ApplyThinkingLevel`, `ApplyToolFilter`, `EmitLogicalEvent`, and unqualified matches/constructors for those names outside `core_effects.rs`. For engine-input checks, reject `EngineInput::*`, direct imports of `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, `CancelTurn`, and unqualified matches/constructors for those names outside the allowed agent turn adapter files.

### Final validation traceability additions

- `SubmitUserPrompt` seam: rail parser hardening also rejects `EngineInput::*`, direct imports of `SubmitUserPrompt`, and unqualified `SubmitUserPrompt` constructors outside `crates/clankers-controller/src/core_engine_composition.rs`. Controller prompt-start construction remains isolated to `core_engine_composition.rs`; agent turn files submit prompts by calling the engine reducer through existing engine request effects rather than constructing controller-normalized prompt starts.
- Agent core-type rail applies to every non-test file under `crates/clankers-agent/src/**`, not only `lib.rs` and `turn/**`. Any new agent runtime file importing `clankers_core` or using `CoreInput`, `CoreEffect`, `CoreState`, `CoreOutcome`, `CoreLogicalEvent`, `PromptCompleted`, `PostPromptEvaluation`, `FollowUpDispatchAcknowledged`, `LoopFollowUpCompleted`, or `ToolFilterApplied` fails the rail unless a future spec adds a named exception.
- Final validation bundle includes direct core and engine checks:

```bash
cargo check -Zbuild-std=core,alloc --target thumbv7em-none-eabi -p clankers-core --no-default-features
cargo test -p clankers-engine --lib
cargo test -p clankers-controller core_engine_composition
cargo test -p clankers-controller accepted_engine_prompt
cargo test -p clankers-agent engine_feedback
cargo test -p clankers-controller --test fcis_shell_boundaries
```

### Lifecycle failure and budget handoff details

Follow-up dispatch rejection never creates engine work. If shell dispatch of `AcceptedPromptKind::FollowUp` fails before the engine starts, the adapter feeds `CoreInput::FollowUpDispatchAcknowledged { effect_id, dispatch_status: Rejected(CoreFailure) }` to `clankers-core` and stops. If dispatch was accepted but engine execution later fails or is cancelled, the adapter feeds `CoreInput::LoopFollowUpCompleted { effect_id, completion_status: Failed(CoreFailure) }`; this is used for both `LoopContinuation` and `AutoTest` follow-up sources. User-prompt engine failure/cancellation feeds `CoreInput::PromptCompleted { effect_id, completion_status: Failed(CoreFailure) }`.

`EngineSubmissionPolicy` carries the engine continuation budget explicitly via `model_request_slot_budget`.

Default Clankers assembly passes the existing named constants into this field: normal user prompts use `NORMAL_TURN_MODEL_REQUEST_SLOT_BUDGET`, and loop/auto-test/orchestration follow-up prompts use `ORCHESTRATION_FOLLOW_UP_MODEL_REQUEST_SLOT_BUDGET`.

`accepted_engine_prompt_from_core_outcome` accepts mixed `CoreOutcome::Transitioned` effects only when exactly one engine-submittable effect is present. It ignores/returns alongside non-submittable logical effects for existing `core_effects.rs` helpers to handle. It returns `MissingEnginePromptEffect` when no `StartPrompt` / `RunLoopFollowUp` exists, `ReplayQueuedPromptNeedsFreshCorePrompt` for replay-only outcomes, and `UnexpectedCoreEffect` when more than one engine-submittable effect appears.

Additional tests:

- rejected follow-up dispatch feeds `CoreInput::FollowUpDispatchAcknowledged { Rejected(..) }` and creates no engine input;
- accepted follow-up with engine failure feeds `CoreInput::LoopFollowUpCompleted { Failed(..) }`;
- user prompt engine failure feeds `CoreInput::PromptCompleted { Failed(..) }`;
- budget fields preserve `NORMAL_TURN_MODEL_REQUEST_SLOT_BUDGET` and `ORCHESTRATION_FOLLOW_UP_MODEL_REQUEST_SLOT_BUDGET` through `EngineSubmissionPolicy`.

### Agent feedback rail simplification

The agent feedback source rail has no helper exception. If an allowed non-test agent turn file constructs `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, or `EngineInput::CancelTurn`, that same file must also contain a non-test `clankers_engine::reduce` path. Other agent files are forbidden from constructing those variants.

The final validation bundle includes `./scripts/check-llm-contract-boundary.sh` in addition to the controller FCIS source rail.

### Final rail consistency rules

- `EngineInput::*` glob imports are banned in all non-test files. Allowed files must construct variants with fully qualified `EngineInput::Variant` paths only. `EngineInput::SubmitUserPrompt` is allowed only in `crates/clankers-controller/src/core_engine_composition.rs`; `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, and `EngineInput::CancelTurn` are allowed only in `crates/clankers-agent/src/turn/{mod.rs,execution.rs}`.
- Terminal-policy parser hardening rejects direct imports, unqualified matches, or unqualified constructors for `TurnFinished`, `EngineTerminalFailure`, `StopReason::MaxTokens`, `StopReason::ToolUse`, `StopReason::Stop`, `MaxTokens`, `ToolUse`, `terminal_failure`, `terminal_failure_outcome`, and `terminal_state_with_messages` outside `crates/clankers-engine/src/**`, except non-test `crates/clankers-agent/src/turn/execution.rs` may parse provider stop strings into `StopReason` as an adapter conversion and agent turn tests may assert translation behavior.
- Embedded runtime rail discovery is recursive over every non-test Rust file under `src/modes/event_loop_runner/**`; new helper files inherit the same forbidden core lifecycle and post-prompt policy symbols unless a future spec adds a named exception.


### Final ownership clarifications

Non-prompt engine feedback reduction is agent-adapter-owned only. `crates/clankers-controller/src/core_engine_composition.rs` builds the initial `EngineInput::SubmitUserPrompt` from normalized core prompt data and may reject obviously misrouted feedback in tests, but it does not reduce `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, or `CancelTurn` in production. Those feedback inputs are constructed and reduced only inside `crates/clankers-agent/src/turn/{mod.rs,execution.rs}`.

Mixed `CoreOutcome` handling is two-pass by design. `accepted_engine_prompt_from_core_outcome` scans the outcome only for engine-submittable prompt/follow-up effects and returns that prompt intent or a gate rejection. Callers must still pass the same `CoreOutcome` effects to the existing `core_effects.rs` interpreter helpers for non-submittable logical effects; the prompt gate does not consume or drop those effects.

### Gate-aligned inventory and validation updates

The source rail inventories must exactly include the delta-spec additions:

- Engine forbidden tokens also include `CoreEffectId`, `CompletionStatus`, `CoreFailure`, and `Cancelled`.
- Agent core-type forbidden tokens apply to all non-test `crates/clankers-agent/src/**` and also include `CoreEffectId`, `CompletionStatus`, `CoreFailure`, `Cancelled`, and `PostPromptAction`.
- Core forbidden engine-owned tokens also include `model_request_slot_budget`.

The final validation bundle additionally includes:

```bash
cargo test -p clankers-core pre_engine_cancellation
cargo test -p clankers-controller pre_engine_cancellation
cargo test -p clankers-engine --lib engine_state_fields_are_active
```

`engine_state_fields_are_active` lives in `crates/clankers-engine/src/lib.rs` next to the reducer tests. It maintains an explicit inventory of `EngineState` fields and asserts each remaining field is exercised by at least one reducer transition test or has a short in-test justification string. The inventory must fail when a new field is added without an active-use test/justification.

Pre-engine follow-up cancellation is a distinct adapter path: once core has accepted a loop/auto-test follow-up and dispatch has been accepted, cancellation before engine submission feeds `CoreInput::LoopFollowUpCompleted { completion_status: CompletionStatus::Failed(CoreFailure::Cancelled), effect_id }` to `clankers-core` and creates no `EngineInput::CancelTurn`. Add a controller `pre_engine_cancellation` test for that path in addition to dispatch-rejection and post-engine cancellation tests.

### Remaining design traceability closures

`./scripts/check-llm-contract-boundary.sh` must add `clankers-core` to the forbidden normal-edge crate list for `clankers-engine`. The script must run `cargo tree -p clankers-engine --edges normal` and fail if the output contains `clankers-core v`, printing the matched crate and the full tree excerpt.

Controller `pre_engine_cancellation` tests cover both pre-engine cancellation paths:

- accepted user prompt cancelled before engine submission feeds `CoreInput::PromptCompleted { CompletionStatus::Failed(CoreFailure::Cancelled), effect_id }` and creates no `EngineInput::CancelTurn`;
- accepted loop/auto-test follow-up cancelled before engine submission feeds `CoreInput::LoopFollowUpCompleted { CompletionStatus::Failed(CoreFailure::Cancelled), effect_id }` and creates no `EngineInput::CancelTurn`.

Thinking and disabled-tool preservation are verified by focused controller tests under existing helper seams plus the FCIS rail:

```bash
cargo test -p clankers-controller thinking_effects_remain_core_owned
cargo test -p clankers-controller disabled_tool_effects_remain_core_owned
```

These tests must prove `execute_thinking_effects`, `execute_tool_filter_request_effects`, and `execute_tool_filter_feedback_effects` still route through `core_effects.rs`, preserve shell-visible behavior, and do not move thinking/disabled-tool policy into `clankers-engine` or `clankers-agent`.

### Engine shell-runtime rail and initial reduce seam

`fcis_shell_boundaries.rs` keeps and extends the existing `clankers_engine_surface_stays_shell_native` rail over every non-test file under `crates/clankers-engine/src/**`. In addition to the core-owned tokens above, engine source must reject shell/runtime/app protocol symbols: `clankers_protocol`, `DaemonEvent`, `SessionCommand`, `ControlResponse`, `AttachResponse`, `clanker_tui_types`, `ratatui`, `crossterm`, `portable_pty`, `clankers_provider`, `CompletionProvider`, `ToolRegistry`, `ToolExecutor`, `HookPipeline`, `tokio`, `mpsc`, `oneshot`, `JoinHandle`, `sleep`, `Sleep`, `reqwest`, `redb`, and `iroh`.

Initial accepted prompt reduction remains in the agent turn host adapter. `core_engine_composition.rs` produces `EngineSubmissionPlan`; the production consumer is `crates/clankers-agent/src/turn/mod.rs::run_turn_loop`, via the existing engine turn entry path, which feeds `EngineInput::SubmitUserPrompt` to `clankers_engine::reduce` and owns resulting engine state/effects. Add a focused test:

```bash
cargo test -p clankers-agent accepted_prompt_submission_reduces_engine
```

That test must prove normalized core prompt data becomes an `EngineInput::SubmitUserPrompt`, is reduced through `clankers_engine::reduce`, and yields the expected pending model request effect before provider I/O starts.
