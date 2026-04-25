# no-std-functional-core Specification

## Purpose
Define the portable `clankers-core` functional core boundary for deterministic prompt, loop, thinking, and tool-filter orchestration so `clankers-controller` owns one no-std-safe policy layer with explicit effects while `clankers-agent` integrates through shell-native controller adapters and continuous parity/boundary verification.
## Requirements
### Requirement: Clankers MUST provide a portable no-std core crate

The system MUST provide a workspace crate named `clankers-core` that compiles with `#![no_std]` and `alloc` and does not depend on Tokio, filesystem, networking, terminal, database, or process APIs. `SessionController` remains the owner of runtime core state for the migrated prompt-lifecycle slice, while `clankers-agent` runtime APIs remain shell-native and interact with core-owned lifecycle work only through controller-owned adapters.
r[no.std.functional.core.portable.crate]

#### Scenario: no-std build succeeds
r[no.std.functional.core.portable.crate.no-std-build-succeeds]

- **WHEN** `clankers-core` is built in its `no_std` configuration
- **THEN** compilation succeeds without linking `std`
- **THEN** the crate exposes the state, input, and effect types needed by the extracted orchestration slice

#### Scenario: std shells consume the same crate through controller adapters
r[no.std.functional.core.portable.crate.std-shells-consume-same-crate]

- **WHEN** `clankers-controller` depends on `clankers-core`
- **THEN** `SessionController` owns the authoritative migrated-slice `clankers-core` state and uses the core state transition APIs from ordinary `std` builds
- **THEN** `clankers-agent` executes controller-requested shell work and returns shell-native feedback without adopting `clankers-core` runtime/public API types for this slice
- **THEN** those shells do not keep a forked orchestration implementation outside the core for the migrated slice

### Requirement: Session command and prompt lifecycle decisions MUST be deterministic core transitions
ID: no.std.functional.core.deterministic.transitions
The system MUST move the initial session command and prompt lifecycle slice into pure core transitions. That slice MUST cover `SessionCommand::Prompt`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, loop-state changes used by `StartLoop` / `StopLoop`, the prompt-completion / post-prompt follow-up inputs currently driven by `notify_prompt_done()` and `check_post_prompt()`, and the embedded prompt-lifecycle ordering decisions that currently live in `src/modes/event_loop_runner/mod.rs` after prompt completion.

#### Scenario: same state and input produce same transition
ID: no.std.functional.core.deterministic.transitions.same-state-same-transition
- **WHEN** the core transition function is called twice with identical prior state and identical input
- **THEN** it returns identical next state and identical effect plans both times

#### Scenario: loop and post-prompt entrypoints are included in the slice
ID: no.std.functional.core.deterministic.transitions.loop-and-post-prompt-entrypoints-are-included
- **WHEN** the shell routes `StartLoop`, `StopLoop`, `notify_prompt_done()`, `check_post_prompt()`, or embedded prompt-result handling through the migrated slice
- **THEN** `clankers-core` owns the state transition logic for those entrypoints
- **THEN** the shell only translates the entrypoint into explicit input data and executes returned work plans

#### Scenario: post-prompt decisions use explicit core data
ID: no.std.functional.core.deterministic.transitions.post-prompt-decisions-use-explicit-core-data
- **WHEN** the core evaluates the transitions currently driven by `notify_prompt_done()`, `check_post_prompt()`, or embedded prompt-result handling
- **THEN** prompt completion outcome, active loop state, pending follow-up state, auto-test enabled state, auto-test command presence, auto-test in-progress state, queued user-prompt presence, and follow-up dispatch outcome are all supplied as explicit input or state data
- **THEN** the core does not infer those facts from shell-only ambient state

#### Scenario: transition logic has no ambient runtime dependencies
ID: no.std.functional.core.deterministic.transitions.transition-logic-has-no-ambient-runtime-dependencies
- **WHEN** the core decides whether a prompt can start, whether loop state changes, whether queued prompt replay wins over controller follow-up, or whether tool filters change
- **THEN** the decision depends only on explicit input data passed to the core
- **THEN** it does not read clocks, environment variables, filesystem state, sockets, channels, or async handles directly

#### Scenario: embedded queued-prompt precedence is explicit
ID: no.std.functional.core.deterministic.transitions.embedded-queued-prompt-precedence-is-explicit
- **WHEN** an embedded prompt finishes while both a queued user prompt and a controller-generated post-prompt follow-up are possible
- **THEN** the migrated prompt-lifecycle slice chooses the precedence order through explicit transition logic
- **THEN** the runtime shell does not re-derive that precedence with local branching after the controller has already selected the next action

#### Scenario: loop continuation outranks auto-test when both are eligible
ID: no.std.functional.core.deterministic.transitions.loop-continuation-outranks-auto-test-when-both-eligible
- **WHEN** prompt completion leaves an active loop eligible for continuation and auto-test is also enabled with a command present
- **THEN** the migrated prompt-lifecycle slice selects loop continuation rather than auto-test for that step
- **THEN** auto-test stays suppressed until loop continuation no longer claims the next action

### Requirement: Core MUST request migrated shell work through explicit effects
ID: no.std.functional.core.explicit.effects
The system MUST encode prompt start, logical event emission, loop follow-up work, filtered-tool rebuild work, and any other runtime work migrated in this first slice as effect values returned from core transitions.

#### Scenario: prompt request yields shell work
ID: no.std.functional.core.explicit.effects.prompt-request-yields-shell-work
- **WHEN** a session input asks to start a prompt in a state where prompting is allowed
- **THEN** the core returns an effect plan describing the runtime work that the shell must perform
- **THEN** the shell does not re-derive orchestration policy before executing that plan

#### Scenario: migrated shell results re-enter as explicit inputs
ID: no.std.functional.core.explicit.effects.migrated-shell-results-re-enter-as-explicit-inputs
- **WHEN** prompt completion, loop follow-up completion, or filtered-tool rebuild application completes in the shell
- **THEN** the shell feeds the resulting data back into the core as a new input
- **THEN** the core derives the next state and follow-up effects from that explicit data alone

### Requirement: Effect plans and shell feedback MUST be explicitly correlated
ID: no.std.functional.core.correlated.feedback
Any `clankers-core` effect that expects shell feedback MUST carry an explicit correlation token or pending-work identity, and matching shell feedback inputs MUST return that identity to the core. Follow-up dispatch acknowledgement and follow-up prompt completion MUST remain distinct lifecycle stages when both matter to state progression.

#### Scenario: correlation identity is core-owned
ID: no.std.functional.core.correlated.feedback.correlation-identity-is-core-owned
- **WHEN** the core emits an effect that expects shell feedback
- **THEN** the correlation token or pending-work identity is derived from explicit core state and deterministic transition input
- **THEN** the shell echoes that identity back instead of minting ambient runtime IDs

#### Scenario: pending-work identities stay unambiguous
ID: no.std.functional.core.correlated.feedback.pending-work-identities-stay-unambiguous
- **WHEN** the first slice has feedback-bearing work pending
- **THEN** the core tracks at most one pending prompt, one pending loop follow-up, and one pending tool-filter rebuild at a time
- **THEN** each pending item carries a core-owned identity unique across the session's feedback-bearing work namespace

#### Scenario: matching shell feedback is accepted
ID: no.std.functional.core.correlated.feedback.matching-shell-feedback-is-accepted
- **WHEN** the shell feeds back a completion input whose correlation token matches pending work in the current core state
- **THEN** that identity is unambiguous across all simultaneously pending feedback-bearing work in the first slice
- **THEN** the core may accept that input and advance the state machine

#### Scenario: prompt completion is correlated
ID: no.std.functional.core.correlated.feedback.prompt-completion-is-correlated
- **WHEN** the shell reports completion of prompt work emitted by the core
- **THEN** that completion input carries the originating effect identity plus explicit completion outcome data
- **THEN** the core matches it against pending prompt state before advancing

#### Scenario: failed prompt completion carries explicit failure data
ID: no.std.functional.core.correlated.feedback.failed-prompt-completion-carries-explicit-failure-data
- **WHEN** the shell reports failed prompt completion
- **THEN** the completion input includes the explicit failure outcome needed for the core to clear busy state and decide whether follow-up is suppressed or a loop is failed

#### Scenario: follow-up dispatch acknowledgement is correlated
ID: no.std.functional.core.correlated.feedback.follow-up-dispatch-acknowledgement-is-correlated
- **WHEN** the shell reports acceptance or rejection of dispatch for follow-up work emitted by the core
- **THEN** that dispatch-feedback input carries the originating follow-up effect identity plus explicit dispatch-result data
- **THEN** the core matches it against the pending follow-up dispatch stage before advancing or returning an explicit rejection

#### Scenario: loop follow-up completion is correlated
ID: no.std.functional.core.correlated.feedback.loop-follow-up-completion-is-correlated
- **WHEN** the shell reports completion of loop follow-up work emitted by the core
- **THEN** that completion input carries the originating effect identity plus explicit completion outcome data
- **THEN** the core matches it against pending loop-follow-up state before advancing

#### Scenario: failed loop follow-up completion carries explicit failure data
ID: no.std.functional.core.correlated.feedback.failed-loop-follow-up-completion-carries-explicit-failure-data
- **WHEN** the shell reports failed loop-follow-up completion
- **THEN** the completion input includes the explicit failure outcome needed for the core to terminate or suppress further follow-up without reading shell ambient state

#### Scenario: filtered-tool rebuild application is correlated
ID: no.std.functional.core.correlated.feedback.filtered-tool-rebuild-application-is-correlated
- **WHEN** the shell reports filtered-tool rebuild application for disabled tools
- **THEN** that completion input carries the originating effect identity plus the applied disabled-tool set/result data
- **THEN** the core matches it against pending tool-filter state before advancing

#### Scenario: mismatched shell feedback is rejected
ID: no.std.functional.core.correlated.feedback.mismatched-shell-feedback-is-rejected
- **WHEN** the shell feeds back a completion input whose correlation token does not match pending work in the current core state
- **THEN** the core returns an explicit rejection describing the mismatch
- **THEN** previously valid state remains unchanged

#### Scenario: wrong-stage or out-of-order lifecycle feedback is rejected
ID: no.std.functional.core.correlated.feedback.wrong-stage-or-out-of-order-lifecycle-feedback-is-rejected
- **WHEN** the shell reports feedback for the wrong lifecycle stage or in an order the current core state does not permit, even if the correlation identity is otherwise valid
- **THEN** the core returns an explicit rejection describing the invalid stage or ordering
- **THEN** previously valid state remains unchanged

#### Scenario: follow-up dispatch and prompt completion stay distinct
ID: no.std.functional.core.correlated.feedback.follow-up-dispatch-and-prompt-completion-stay-distinct
- **WHEN** the shell accepts or rejects dispatch of a controller-selected follow-up prompt
- **THEN** that dispatch result is reported to the migrated prompt-lifecycle slice as explicit feedback distinct from later prompt-completion feedback
- **THEN** accepting dispatch alone does not clear pending follow-up state, mark loop continuation complete, or synthesize prompt completion before the follow-up prompt actually finishes

### Requirement: Invalid or out-of-order transitions MUST fail explicitly
ID: no.std.functional.core.invalid.transitions
The system MUST reject invalid inputs for the extracted orchestration slice with typed failures rather than panicking or silently mutating unrelated state.

#### Scenario: out-of-order runtime result is rejected
ID: no.std.functional.core.invalid.transitions.out-of-order-runtime-result-is-rejected
- **WHEN** the shell reports a runtime result that does not match the current core state
- **THEN** the core returns an explicit rejection describing the invalid transition
- **THEN** previously valid state remains unchanged

#### Scenario: repeated start while busy is rejected
ID: no.std.functional.core.invalid.transitions.repeated-start-while-busy-is-rejected
- **WHEN** a prompt-start input arrives while the core already marks the session busy
- **THEN** the core does not schedule duplicate prompt execution effects
- **THEN** it returns an explicit busy rejection
- **THEN** previously valid state remains unchanged

#### Scenario: start-loop while loop already active is rejected
ID: no.std.functional.core.invalid.transitions.start-loop-while-loop-already-active-is-rejected
- **WHEN** `StartLoop` arrives while the core already tracks an active loop
- **THEN** the core returns an explicit loop-already-active rejection
- **THEN** previously valid state remains unchanged

#### Scenario: stop-loop with no active loop is rejected
ID: no.std.functional.core.invalid.transitions.stop-loop-with-no-active-loop-is-rejected
- **WHEN** `StopLoop` arrives while the core tracks no active loop
- **THEN** the core returns an explicit loop-not-active rejection
- **THEN** previously valid state remains unchanged

#### Scenario: loop-control input while follow-up is pending is rejected
ID: no.std.functional.core.invalid.transitions.loop-control-input-while-follow-up-is-pending-is-rejected
- **WHEN** `StartLoop` or `StopLoop` arrives while loop follow-up work is still pending for the current core state
- **THEN** the core returns an explicit pending-follow-up rejection
- **THEN** previously valid state remains unchanged

#### Scenario: disabled-tools update with stale pending rebuild is rejected
ID: no.std.functional.core.invalid.transitions.disabled-tools-update-with-stale-pending-rebuild-is-rejected
- **WHEN** `SetDisabledTools` arrives while a prior tool-filter rebuild slot is still pending
- **THEN** the core returns an explicit stale-slot rejection
- **THEN** previously valid state remains unchanged

### Requirement: std shells MUST preserve migrated behavior through the core
ID: no.std.functional.core.shell.parity
The system MUST route the extracted orchestration slice through `clankers-core` while preserving the current shell-visible behavior of controller, agent, and embedded runtime integrations for that slice.

#### Scenario: prompt and prompt-completion behavior remain aligned
ID: no.std.functional.core.shell.parity.prompt-and-prompt-completion-behavior-remain-aligned
- **WHEN** existing controller or agent tests exercise prompt start, prompt completion feedback, and busy gating through the migrated `std` shells
- **THEN** prompt start still marks the session busy only when allowed
- **THEN** successful prompt completion still clears busy state, emits no dedicated completion acknowledgement event, and selects only one of queued user-prompt replay, `PostPromptAction::None`, `PostPromptAction::ContinueLoop`, or `PostPromptAction::RunAutoTest` as the next-action surface
- **THEN** failed prompt completion still clears busy state, emits no success acknowledgement, suppresses `PostPromptAction::ContinueLoop` and `PostPromptAction::RunAutoTest`, and preserves loop-related failure notification through the error `DaemonEvent::SystemMessage` category when an active loop is failed
- **THEN** after failed prompt completion, embedded mode may replay an already queued user prompt but otherwise selects no further next action
- **THEN** repeated prompt start while busy still yields one explicit rejection, preserves the error `DaemonEvent::SystemMessage` acknowledgement category, and produces no duplicate prompt-start effect

#### Scenario: thinking-level behavior remains aligned
ID: no.std.functional.core.shell.parity.thinking-level-behavior-remains-aligned
- **WHEN** the shell routes `SetThinkingLevel` or `CycleThinkingLevel` through the migrated slice
- **THEN** `CycleThinkingLevel` still follows the current order `Off → Low → Medium → High → Max → Off`
- **THEN** valid thinking-level changes still update the effective thinking configuration and emit the `DaemonEvent::SystemMessage` acknowledgement category
- **THEN** invalid `SetThinkingLevel` inputs still leave state unchanged and emit the error `DaemonEvent::SystemMessage` category

#### Scenario: disabled-tool behavior remains aligned
ID: no.std.functional.core.shell.parity.disabled-tool-behavior-remains-aligned
- **WHEN** the shell routes `SetDisabledTools` through the migrated slice
- **THEN** the core records the requested disabled-tool set when the request is accepted
- **THEN** filtered-tool rebuild application still occurs before the shell reports `ToolFilterApplied` completion
- **THEN** after successful rebuild application, the shell emits `DaemonEvent::DisabledToolsChanged` followed by the `DaemonEvent::SystemMessage` acknowledgement category as current behavior

#### Scenario: disabled-tool stale-slot rejection remains aligned
ID: no.std.functional.core.shell.parity.disabled-tool-stale-slot-rejection-remains-aligned
- **WHEN** the shell routes `SetDisabledTools` while a prior tool-filter rebuild slot is still pending
- **THEN** the disabled-tool state remains unchanged
- **THEN** the shell emits the error `DaemonEvent::SystemMessage` acknowledgement category
- **THEN** the shell emits no `DaemonEvent::DisabledToolsChanged` event for the rejected update

#### Scenario: post-prompt follow-up behavior remains aligned
ID: no.std.functional.core.shell.parity.post-prompt-follow-up-behavior-remains-aligned
- **WHEN** the shell routes `notify_prompt_done()` or `check_post_prompt()` through the migrated slice
- **THEN** when both a queued user prompt and a controller-generated post-prompt follow-up are eligible after prompt completion, queued user-prompt replay still wins for that step and controller-generated follow-up dispatch does not run first
- **THEN** successful loop follow-up completion still clears the pending follow-up slot and advances visible loop iteration or active-state transitions only after the follow-up prompt actually completes
- **THEN** non-loop follow-up completion finishes with no extra acknowledgement event
- **THEN** post-prompt next-action selection still stays within queued user-prompt replay plus `PostPromptAction::None`, `PostPromptAction::ContinueLoop`, and `PostPromptAction::RunAutoTest`

#### Scenario: failed loop follow-up completion remains aligned
ID: no.std.functional.core.shell.parity.failed-loop-follow-up-completion-remains-aligned
- **WHEN** the shell routes failed follow-up completion through the migrated slice
- **THEN** the pending follow-up slot is cleared and no additional follow-up is scheduled from that failed completion
- **THEN** any loop owned by that follow-up remains in its failed or inactive visible state
- **THEN** the shell preserves error `DaemonEvent::SystemMessage` for the failed follow-up notification

#### Scenario: loop-control behavior remains aligned
ID: no.std.functional.core.shell.parity.loop-control-behavior-remains-aligned
- **WHEN** the shell routes `StartLoop` or `StopLoop` through the migrated slice
- **THEN** successful `StartLoop` still sets the active or visible loop state (`active_loop_id` present and loop shown active) and emits no immediate acknowledgement event
- **THEN** successful `StopLoop` still preserves the current visible loop-state transition plus the success `DaemonEvent::SystemMessage` acknowledgement category
- **THEN** explicit rejection cases (`LoopAlreadyActive`, `LoopNotActive`, and pending-follow-up rejection) leave state unchanged and preserve the error `DaemonEvent::SystemMessage` acknowledgement category

#### Scenario: embedded prompt lifecycle behavior remains aligned
ID: no.std.functional.core.shell.parity.embedded-prompt-lifecycle-behavior-remains-aligned
- **WHEN** embedded mode processes `TaskResult::PromptDone` for a prompt started through the migrated controller/core path
- **THEN** queued user-prompt replay still takes precedence over controller-generated post-prompt follow-up when both are available
- **THEN** when no queued user prompt exists, the next post-prompt action comes only from controller-owned migrated prompt-lifecycle output rather than TUI-local policy branches
- **THEN** after failed prompt completion, queued user-prompt replay may still run if one is already pending, but controller-generated loop continuation and auto-test follow-up do not dispatch
- **THEN** rejected or failed follow-up dispatch is reported back through explicit controller feedback instead of being treated as a successful follow-up completion

#### Scenario: lifecycle feedback rejection behavior remains aligned
ID: no.std.functional.core.shell.parity.lifecycle-feedback-rejection-behavior-remains-aligned
- **WHEN** controller adapters receive mismatched, wrong-stage, or out-of-order lifecycle feedback for the migrated slice
- **THEN** the shell surfaces the explicit rejection through the existing error `DaemonEvent::SystemMessage` category
- **THEN** previously valid state remains unchanged
- **THEN** rejected feedback does not synthesize queued-prompt replay, follow-up dispatch, or loop advancement
- **WHEN** embedded adapters receive mismatched, wrong-stage, or out-of-order lifecycle feedback for the migrated slice
- **THEN** standalone TUI surfaces the explicit rejection through the existing `App::push_system(..., true)` path used by `src/modes/event_loop_runner/mod.rs` rather than inventing a daemon-only event type
- **THEN** previously valid state remains unchanged
- **THEN** rejected feedback does not synthesize queued-prompt replay, follow-up dispatch, or loop advancement

### Requirement: Future deterministic extractions MUST reuse the same boundary
Any later orchestration logic moved under this capability MUST enter `clankers-core` as explicit state, input, and effect transformations when it does not require direct I/O. Shell-specific protocol, runtime, transport, and terminal event-loop types MUST stay in adapter code, and reusable host-facing harness semantics MUST be staged through `clankers-engine` rather than being left controller-specific. Engine-owned model/tool turn policy MUST NOT move into `clankers-core` by accident; any future downward migration requires explicit no-std-core requirements, state, tests, and boundary rails.
r[no.std.functional.core.future.extraction.boundary]

#### Scenario: future pure logic moves into the core
r[no.std.functional.core.future.extraction.boundary.future-pure-logic-moves-into-the-core]
- **WHEN** a later deterministic orchestration rule is migrated under the `no-std-functional-core` capability
- **THEN** that rule is implemented in `clankers-core`
- **THEN** shell adapters are limited to translation and effect execution

#### Scenario: shell-native types stay outside the core boundary
r[no.std.functional.core.future.extraction.boundary.shell-native-types-stay-outside-the-core-boundary]
- **WHEN** the migrated slice needs `DaemonEvent`, `AgentEvent`, Tokio, terminal, or transport-specific values
- **THEN** those shell-native values are created and consumed in adapter code
- **THEN** raw shell-native or protocol-native types do not appear in exported `clankers-core` boundary types including state, input, effect, outcome, or error types

#### Scenario: reusable engine boundary stages future extractions
r[no.std.functional.core.future.extraction.boundary.reusable-engine-boundary-stages-future-extractions]
- **WHEN** Clankers migrates another reusable orchestration slice after the initial prompt-lifecycle extraction
- **THEN** the first host-facing landing zone for that reusable logic is `clankers-engine`
- **THEN** controller and agent shells adapt the engine boundary instead of keeping controller-only reusable policy

#### Scenario: turn orchestration extraction targets the embeddable engine path
r[no.std.functional.core.future.extraction.boundary.turn-orchestration-extraction-targets-the-embeddable-engine-path]
- **WHEN** Clankers migrates prompt, model, tool, retry, or continuation policy that belongs in an embedded agent harness
- **THEN** that migration is planned as `clankers-agent` and `clankers-controller` shell work around an engine-owned contract
- **THEN** deterministic portions remain eligible for later downward movement into `clankers-core` only through a separate explicit no-std-core migration contract with tests and source rails

#### Scenario: controller shell execution stays centralized
r[no.std.functional.core.future.extraction.boundary.controller-shell-execution-stays-centralized]
- **WHEN** migrated prompt-lifecycle effects are interpreted in `std` shells
- **THEN** `crates/clankers-controller/src/core_effects.rs` is the single controller-owned interpreter for that slice's effect semantics
- **THEN** `command.rs`, `auto_test.rs`, embedded runtime files, and agent adapters do not keep their own prompt-lifecycle effect interpreters or re-derive the same shell semantics

#### Scenario: agent runtime APIs stay shell-native
r[no.std.functional.core.future.extraction.boundary.agent-runtime-apis-stay-shell-native]
- **WHEN** migrated prompt-lifecycle effects or results cross from controller adapters into `clankers-agent`
- **THEN** core-type translation happens in controller-owned adapters
- **THEN** `clankers-agent` runtime and public APIs stay shell-native for this slice and do not adopt `clankers-core` types directly

#### Scenario: interactive shell work stays outside the core
r[no.std.functional.core.future.extraction.boundary.interactive-shell-work-stays-outside-the-core]
- **WHEN** the migrated prompt-lifecycle slice touches actual prompt sending, plugin dispatch, clipboard or editor work, terminal rendering, or Tokio channel coordination
- **THEN** those behaviors remain in shell adapters rather than `clankers-core`
- **THEN** the core emits plain-data intents only and does not perform those interactive side effects directly

#### Scenario: embedded event loop runner stays adapter-only
r[no.std.functional.core.future.extraction.boundary.embedded-event-loop-runner-stays-adapter-only]
- **WHEN** the embedded prompt-lifecycle slice is migrated after prompt completion
- **THEN** `src/modes/event_loop_runner/mod.rs` and nearby runtime helpers stay limited to channel I/O, UI updates, and dispatch of controller-selected work
- **THEN** those runtime files do not keep their own queued-prompt precedence rules, follow-up completion synthesis, or duplicated post-prompt state-transition logic

### Requirement: The no-std boundary and parity contract MUST be continuously verified
ID: no.std.functional.core.continuous.verification
The system MUST keep dedicated verification rails for the `no-std-functional-core` capability so architectural and behavioral regressions are caught before acceptance.

#### Scenario: dedicated no-std compile rail runs
ID: no.std.functional.core.continuous.verification.dedicated-no-std-compile-rail-runs
- **WHEN** validation runs for the capability
- **THEN** it executes `cargo check -p clankers-core --no-default-features --target thumbv7em-none-eabi` or an equivalent repo-defined bare-metal no-std compile rail
- **THEN** failure blocks acceptance of the change

#### Scenario: banned dependency and API rail runs
ID: no.std.functional.core.continuous.verification.banned-dependency-and-api-rail-runs
- **WHEN** validation runs for the capability
- **THEN** it executes `scripts/check-clankers-core-boundary.sh` as the persistent repo boundary check over `clankers-core` dependencies and source imports for Tokio, terminal/TUI, networking, database, filesystem, and process APIs
- **THEN** failure blocks acceptance of the change

#### Scenario: public core surface shell-native-type rail runs
ID: no.std.functional.core.continuous.verification.public-core-surface-shell-native-type-rail-runs
- **WHEN** validation runs for the capability
- **THEN** it executes `scripts/check-clankers-core-surface.sh` over exported `clankers-core` boundary types to reject shell-native or protocol-native type leakage outside adapter code
- **THEN** failure blocks acceptance of the change

#### Scenario: dedicated determinism check runs
ID: no.std.functional.core.continuous.verification.dedicated-determinism-check-runs
- **WHEN** validation runs for the capability
- **THEN** it replays identical migrated-slice state or input pairs twice and asserts identical next state plus identical effect plans
- **THEN** failure blocks acceptance of the change

#### Scenario: reducer coverage traces each required behavior
ID: no.std.functional.core.continuous.verification.reducer-coverage-traces-each-required-behavior
- **WHEN** validation runs for the capability
- **THEN** reducer tests cover busy gating, prompt start, prompt completion, queued-prompt precedence, follow-up dispatch feedback, loop continuation, the simultaneous-eligibility case where loop continuation must outrank auto-test, failed loop follow-up completion, `StartLoop` / `StopLoop`, pending-follow-up loop-control rejection, `notify_prompt_done()`, `check_post_prompt()`, thinking changes, tool-filter changes, mismatched-feedback rejection, wrong-stage lifecycle-feedback rejection, and out-of-order runtime-result rejection for the migrated slice
- **THEN** failure blocks acceptance of the change

#### Scenario: shell parity coverage traces each adapter seam
ID: no.std.functional.core.continuous.verification.shell-parity-coverage-traces-each-adapter-seam
- **WHEN** validation runs for the capability
- **THEN** controller parity tests cover `crates/clankers-controller/src/command.rs`, `crates/clankers-controller/src/auto_test.rs`, `crates/clankers-controller/src/core_effects.rs`, queued user-prompt replay winning over controller-generated follow-up when both are eligible, the simultaneous-eligibility case where loop continuation must outrank auto-test, prompt-completion feedback, follow-up dispatch feedback, failed loop follow-up completion, filtered-tool rebuild application, mismatched, wrong-stage, or out-of-order lifecycle feedback surfacing through error `DaemonEvent::SystemMessage`, one `SessionController::handle_command(SessionCommand::Prompt { .. })` shell seam, an explicit `SetThinkingLevel` regression, an explicit `SetDisabledTools` regression, an explicit `CycleThinkingLevel` regression, and an explicit `StartLoop` / `StopLoop` regression
- **THEN** embedded runtime parity tests cover `src/modes/event_loop_runner/mod.rs` and assert queued-prompt precedence, controller-owned next-action dispatch, explicit follow-up-dispatch rejection handling, mismatched, wrong-stage, or out-of-order rejection surfacing through `App::push_system(..., true)`, and no premature loop-follow-up completion
- **THEN** agent parity tests cover `crates/clankers-agent/src/turn/mod.rs` and one migrated-slice adapter regression in `crates/clankers-agent/src/lib.rs`
- **THEN** failure blocks acceptance of the change

#### Scenario: anti-fork review confirms single migrated policy path
ID: no.std.functional.core.continuous.verification.anti-fork-review-confirms-single-migrated-policy-path
- **WHEN** acceptance review runs for the capability
- **THEN** `clankers-agent`, `clankers-controller`, and embedded runtime files no longer keep duplicated policy logic for the migrated slice outside `clankers-core`
- **THEN** any remaining shell code for the slice is limited to translation and effect execution

### Requirement: Embedded runtime feedback adapters MUST stay shell-native
ID: no.std.functional.core.embedded.feedback.shell.native.adapters
The system MUST expose controller-owned shell-native adapters for embedded prompt completion and follow-up dispatch/completion feedback so root runtime files do not construct `clankers_core` feedback types directly for the migrated slice.

#### Scenario: controller-owned pending-work identities cross the shell boundary
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.controller-owned-pending-work-identities-cross-the-shell-boundary
- **WHEN** embedded runtime code receives `PostPromptAction::ContinueLoop` or `PostPromptAction::RunAutoTest`
- **THEN** the correlation token returned to runtime code is a controller-owned pending-work identity rather than a raw `clankers_core::CoreEffectId`
- **THEN** conversion back to core-owned correlation data happens inside `crates/clankers-controller`

#### Scenario: embedded runtime avoids direct core feedback construction
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.embedded-runtime-avoids-direct-core-feedback-construction
- **WHEN** `src/modes/event_loop_runner/mod.rs` reports accepted or rejected follow-up dispatch, successful or failed embedded prompt completion, or successful or failed follow-up completion
- **THEN** it calls controller-owned shell-native APIs
- **THEN** it does not construct `clankers_core::CompletionStatus`, `clankers_core::FollowUpDispatchStatus`, `clankers_core::CoreFailure`, or raw core correlation IDs directly in non-test code

#### Scenario: embedded feedback behavior remains aligned
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.embedded-feedback-behavior-remains-aligned
- **WHEN** embedded shells report accepted or rejected follow-up dispatch or successful or failed prompt/follow-up completion through the shell-native controller adapters for either `PostPromptAction::ContinueLoop` or `PostPromptAction::RunAutoTest`
- **THEN** accepted dispatch plus successful follow-up completion still advances loop state only after the follow-up prompt actually finishes
- **THEN** rejected dispatch, failed follow-up completion, and failed prompt completion still preserve queued-prompt replay rules plus loop-failure and error surfacing behavior on both follow-up branches
- **THEN** rejections still leave previously valid state unchanged

#### Scenario: fcis boundary rail detects core-type leakage in embedded runtime code
ID: no.std.functional.core.embedded.feedback.shell.native.adapters.fcis-boundary-rail-detects-core-type-leakage-in-embedded-runtime-code
- **WHEN** the FCIS shell-boundary rail inventories non-test runtime references in `src/modes/event_loop_runner/mod.rs`
- **THEN** it finds no `clankers_core` path usage at all in that file's non-test code
- **THEN** any reintroduced runtime core-type leakage fails the deterministic rail

### Requirement: Core lifecycle cancellation MUST cover pre-engine submission

The no-std core MUST own cancellation feedback for prompt or follow-up lifecycle work that has been accepted by core but has not yet been submitted to `clankers-engine`.
r[no.std.functional.core.pre.engine.cancellation]

#### Scenario: accepted prompt cancelled before engine submission
r[no.std.functional.core.pre.engine.cancellation.prompt-before-engine]
- **WHEN** a prompt has pending core lifecycle work and the adapter cancels before creating an engine submission
- **THEN** the adapter reports `CoreInput::PromptCompleted` with `CompletionStatus::Failed(CoreFailure::Cancelled)` and the matching core effect ID
- **THEN** `clankers-core` clears the pending prompt and busy lifecycle state and emits the same lifecycle effects as failed prompt completion
- **THEN** mismatched effect IDs are rejected by `clankers-core`

#### Scenario: accepted follow-up cancelled before engine submission
r[no.std.functional.core.pre.engine.cancellation.follow-up-before-engine]
- **WHEN** a loop or auto-test follow-up has pending core lifecycle work and the adapter cancels before creating an engine submission
- **THEN** the adapter reports `CoreInput::LoopFollowUpCompleted` with `CompletionStatus::Failed(CoreFailure::Cancelled)` and the matching follow-up effect ID
- **THEN** `clankers-core` clears or advances follow-up lifecycle state according to the existing failed follow-up completion policy
- **THEN** mismatched or wrong-stage follow-up cancellation is rejected by `clankers-core`

#### Scenario: shell parity preserves cancellation behavior
r[no.std.functional.core.pre.engine.cancellation.shell-parity]
- **WHEN** controller or embedded adapters handle pre-engine cancellation for accepted prompt or follow-up lifecycle work
- **THEN** user-visible busy, loop, auto-test, and queued-prompt behavior matches the existing failed lifecycle completion behavior
- **THEN** no `clankers-engine` cancellation input is created for work that never reached the engine

