## MODIFIED Requirements

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

## ADDED Requirements

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
