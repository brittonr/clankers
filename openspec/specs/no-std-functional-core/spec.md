# no-std-functional-core Specification

## Purpose
Define the portable `clankers-core` functional core boundary for deterministic prompt, loop, thinking, and tool-filter orchestration so `clankers-controller` and `clankers-agent` share one no-std-safe policy layer with explicit effects and continuous parity/boundary verification.
## Requirements
### Requirement: Clankers MUST provide a portable no-std core crate
ID: no.std.functional.core.portable.crate
The system MUST provide a workspace crate named `clankers-core` that compiles with `#![no_std]` and `alloc` and does not depend on Tokio, filesystem, networking, terminal, database, or process APIs.

#### Scenario: no-std build succeeds
ID: no.std.functional.core.portable.crate.no-std-build-succeeds
- **WHEN** `clankers-core` is built in its `no_std` configuration
- **THEN** compilation succeeds without linking `std`
- **THEN** the crate exposes the state, input, and effect types needed by the extracted orchestration slice

#### Scenario: std shells consume the same crate
ID: no.std.functional.core.portable.crate.std-shells-consume-same-crate
- **WHEN** `clankers-agent` or `clankers-controller` depends on `clankers-core`
- **THEN** `SessionController` owns the authoritative migrated-slice `clankers-core` state and uses the core state transition APIs from ordinary `std` builds
- **THEN** `clankers-agent` uses the same core crate types and effect/input contracts, executes controller-owned effects, and returns explicit feedback without maintaining a second authoritative reducer for the migrated slice
- **THEN** those shells do not keep a forked orchestration implementation outside the core for the migrated slice

### Requirement: Session command and prompt lifecycle decisions MUST be deterministic core transitions
ID: no.std.functional.core.deterministic.transitions
The system MUST move the initial session command and prompt lifecycle slice into pure core transitions. That initial slice MUST cover `SessionCommand::Prompt`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, loop-state changes used by `StartLoop` / `StopLoop`, and the prompt-completion / post-prompt follow-up inputs currently driven by `notify_prompt_done()` and `check_post_prompt()`.

#### Scenario: same state and input produce same transition
ID: no.std.functional.core.deterministic.transitions.same-state-same-transition
- **WHEN** the core transition function is called twice with identical prior state and identical input
- **THEN** it returns identical next state and identical effect plans both times

#### Scenario: loop and post-prompt entrypoints are included in the slice
ID: no.std.functional.core.deterministic.transitions.loop-and-post-prompt-entrypoints-are-included
- **WHEN** the shell routes `StartLoop`, `StopLoop`, `notify_prompt_done()`, or `check_post_prompt()` behavior through the migrated slice
- **THEN** the core owns the state transition logic for those entrypoints
- **THEN** the shell only translates the entrypoint into `CoreInput` and executes returned `CoreEffect` values

#### Scenario: post-prompt decisions use explicit core data
ID: no.std.functional.core.deterministic.transitions.post-prompt-decisions-use-explicit-core-data
- **WHEN** the core evaluates the transitions currently driven by `notify_prompt_done()` or `check_post_prompt()`
- **THEN** prompt completion outcome, active loop state, pending follow-up state, auto-test enabled state, auto-test command presence, and auto-test in-progress state are all supplied as explicit core input/state data
- **THEN** the core does not infer those facts from shell-only ambient state

#### Scenario: transition logic has no ambient runtime dependencies
ID: no.std.functional.core.deterministic.transitions.transition-logic-has-no-ambient-runtime-dependencies
- **WHEN** the core decides whether a prompt can start, whether loop state changes, or whether tool filters change
- **THEN** the decision depends only on explicit input data passed to the core
- **THEN** it does not read clocks, environment variables, filesystem state, sockets, or async handles directly

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
Any `clankers-core` effect that expects shell feedback MUST carry an explicit correlation token or pending-work identity, and matching shell feedback inputs MUST return that identity to the core.

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
The system MUST route the extracted orchestration slice through `clankers-core` while preserving the current shell-visible behavior of controller and agent integrations for that slice.

#### Scenario: prompt and prompt-completion behavior remain aligned
ID: no.std.functional.core.shell.parity.prompt-and-prompt-completion-behavior-remain-aligned
- **WHEN** existing controller or agent tests exercise prompt start, prompt completion feedback, and busy gating through the migrated `std` shells
- **THEN** prompt start still marks the session busy only when allowed
- **THEN** successful prompt completion still clears busy state, emits no dedicated completion acknowledgement event, and selects only `PostPromptAction::{None, ContinueLoop, RunAutoTest}` as the next-action surface
- **THEN** failed prompt completion still clears busy state, suppresses `PostPromptAction::{ContinueLoop, RunAutoTest}`, emits no success acknowledgement, and preserves loop-related failure notification through the error `DaemonEvent::SystemMessage` category when an active loop is failed
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
- **THEN** successful loop follow-up completion still clears the pending follow-up slot and advances visible loop iteration/active-state transitions for loop continuation
- **THEN** non-loop follow-up completion finishes with no extra acknowledgement event
- **THEN** post-prompt next-action selection still stays within the current `PostPromptAction::{None, ContinueLoop, RunAutoTest}` categories

#### Scenario: failed loop follow-up completion remains aligned
ID: no.std.functional.core.shell.parity.failed-loop-follow-up-completion-remains-aligned
- **WHEN** the shell routes failed `LoopFollowUpCompleted` through the migrated slice
- **THEN** the pending follow-up slot is cleared and no additional follow-up is scheduled from that failed completion
- **THEN** any loop owned by that follow-up remains in its failed/inactive visible state
- **THEN** the shell preserves error `DaemonEvent::SystemMessage` for the failed follow-up notification

#### Scenario: loop-control behavior remains aligned
ID: no.std.functional.core.shell.parity.loop-control-behavior-remains-aligned
- **WHEN** the shell routes `StartLoop` or `StopLoop` through the migrated slice
- **THEN** successful `StartLoop` still sets the active/visible loop state (`active_loop_id` present and loop shown active) and emits no immediate acknowledgement event
- **THEN** successful `StopLoop` still preserves the current visible loop-state transition plus the success `DaemonEvent::SystemMessage` acknowledgement category
- **THEN** explicit rejection cases (`LoopAlreadyActive`, `LoopNotActive`, and pending-follow-up rejection) leave state unchanged and preserve the error `DaemonEvent::SystemMessage` acknowledgement category

### Requirement: Future deterministic extractions MUST reuse the same boundary
ID: no.std.functional.core.future.extraction.boundary
Any later orchestration logic moved under this capability MUST enter `clankers-core` as explicit state, input, and effect transformations when it does not require direct I/O. Shell-specific protocol, runtime, and transport types MUST stay in adapter code.

#### Scenario: future pure logic moves into the core
ID: no.std.functional.core.future.extraction.boundary.future-pure-logic-moves-into-the-core
- **WHEN** a later deterministic orchestration rule is migrated under the `no-std-functional-core` capability
- **THEN** that rule is implemented in `clankers-core`
- **THEN** shell adapters are limited to translation and effect execution

#### Scenario: shell-native types stay outside the core boundary
ID: no.std.functional.core.future.extraction.boundary.shell-native-types-stay-outside-the-core-boundary
- **WHEN** the migrated slice needs `DaemonEvent`, `AgentEvent`, Tokio, or transport-specific values
- **THEN** those shell-native values are created and consumed in adapter code
- **THEN** raw shell-native or protocol-native types do not appear in exported `clankers-core` boundary types including state, input, effect, outcome, or error types

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
- **THEN** it replays identical migrated-slice state/input pairs twice and asserts identical next state plus identical effect plans
- **THEN** failure blocks acceptance of the change

#### Scenario: reducer coverage traces each required behavior
ID: no.std.functional.core.continuous.verification.reducer-coverage-traces-each-required-behavior
- **WHEN** validation runs for the capability
- **THEN** reducer tests cover busy gating, prompt start, prompt completion, loop continuation, `StartLoop` / `StopLoop`, pending-follow-up loop-control rejection, `notify_prompt_done()`, `check_post_prompt()`, thinking changes, tool-filter changes, mismatched-feedback rejection, and out-of-order runtime-result rejection for the migrated slice
- **THEN** failure blocks acceptance of the change

#### Scenario: shell parity coverage traces each adapter seam
ID: no.std.functional.core.continuous.verification.shell-parity-coverage-traces-each-adapter-seam
- **WHEN** validation runs for the capability
- **THEN** controller parity tests cover `crates/clankers-controller/src/command.rs`, `crates/clankers-controller/src/auto_test.rs`, prompt-completion feedback, filtered-tool rebuild application, loop follow-up completion, one `SessionController::handle_command(SessionCommand::Prompt { .. })` shell seam, an explicit `SetDisabledTools` regression, an explicit `CycleThinkingLevel` regression, and an explicit `StartLoop` / `StopLoop` regression
- **THEN** those tests assert required shell-visible ordering where specified, including `DaemonEvent::DisabledToolsChanged` before `DaemonEvent::SystemMessage`, stop-loop visible transition plus success `DaemonEvent::SystemMessage`, and stale-slot rejection with no `DaemonEvent::DisabledToolsChanged`
- **THEN** agent parity tests cover `crates/clankers-agent/src/turn/mod.rs` and one migrated-slice adapter regression in `crates/clankers-agent/src/lib.rs`
- **THEN** failure blocks acceptance of the change

#### Scenario: anti-fork review confirms single migrated policy path
ID: no.std.functional.core.continuous.verification.anti-fork-review-confirms-single-migrated-policy-path
- **WHEN** acceptance review runs for the capability
- **THEN** `clankers-agent` and `clankers-controller` no longer keep duplicated policy logic for the migrated slice outside `clankers-core`
- **THEN** any remaining shell code for the slice is limited to translation and effect execution

