## Context

The first `no-std-functional-core` extraction moved prompt, loop, thinking, and tool-filter policy into `clankers-core`, but embedded mode still keeps one high-risk prompt-lifecycle seam in `src/modes/event_loop_runner/mod.rs`. After `TaskResult::PromptDone`, runtime code still decides whether to replay a queued user prompt, whether to ask the controller for post-prompt work, and whether a follow-up prompt was accepted or failed.

That is dangerous for two reasons:

1. It leaves standalone embedded mode with local policy branches that can drift from controller or daemon behavior.
2. The current follow-up path conflates shell dispatch acceptance with follow-up completion by calling `complete_follow_up(...)` immediately after attempting to queue the next prompt.

This change is intentionally the next FCIS slice, not a full turn-loop rewrite. It targets the deterministic decisions that still remain in embedded runtime code and tightens `crates/clankers-controller/src/core_effects.rs` as the one shell executor for this migrated slice.

## Goals / Non-Goals

**Goals:**
- Move embedded prompt-lifecycle ordering out of `src/modes/event_loop_runner/mod.rs` into explicit controller/core-owned data transformations
- Make queued-prompt precedence and post-prompt follow-up selection explicit and testable
- Separate follow-up dispatch acknowledgement from actual follow-up prompt completion
- Keep `event_loop_runner` as boring shell code: channel I/O, UI updates, and execution of controller-selected plans
- Extend parity and anti-fork rails so embedded mode is covered alongside controller and agent seams

**Non-Goals:**
- Rewrite provider streaming or tool execution in `crates/clankers-agent/src/turn/mod.rs`
- Move terminal drawing, plugin dispatch, clipboard/editor work, or Tokio channel plumbing into `clankers-core`
- Redesign loop UX or auto-test UX
- Broaden this change into a full session-resume or attach parity rewrite beyond the prompt-lifecycle seam it directly shares

## Decisions

### 1. Put post-prompt planning in `clankers-core`, not in controller-local policy code

**Choice:** After prompt completion, embedded mode will ask the controller for one explicit post-prompt plan backed by `clankers-core` transitions. That plan encodes precedence between:
- replaying a queued user prompt,
- dispatching loop continuation,
- dispatching auto-test follow-up,
- doing nothing.

`event_loop_runner` executes that returned plan but does not re-decide precedence, and the controller does not keep an alternate pure planner outside `clankers-core`.

**Rationale:** `handle_task_results()` currently decides locally whether queued prompt replay outranks `check_post_prompt()`. That rule is deterministic and belongs in the shared core boundary. Leaving it in controller-local pure code would still split the ownership contract promised by `no-std-functional-core`.

**Alternatives considered:**
- **Keep precedence in `event_loop_runner` and only document it:** Rejected because drift risk remains and review would still need to reason about hidden runtime policy.
- **Use a controller-local pure wrapper as the permanent home:** Rejected because the spec for this capability requires later deterministic orchestration rules to land in `clankers-core`, not in a second permanent policy layer.

### 2. Treat follow-up dispatch acknowledgement and follow-up prompt completion as separate lifecycle stages

**Choice:** The migrated slice distinguishes between:
- core selecting follow-up work,
- shell reporting whether dispatch of that work was accepted or rejected via a dedicated controller acknowledgment seam (`ack_follow_up_dispatch(follow_up_effect_id, dispatch_status)`),
- later completion of the dispatched follow-up prompt via a distinct correlated controller seam (`complete_dispatched_follow_up(follow_up_effect_id, completion_status)`).

`finish_embedded_prompt(completion_status)` remains the completion path for ordinary embedded prompts. When a follow-up prompt is dispatched successfully, controller adapters retain the originating `follow_up_effect_id` in the pending prompt metadata so the later prompt completion can be translated back into `complete_dispatched_follow_up(...)` with the original follow-up identity and validated against the follow-up stage. In spec terms, `follow-up dispatch acknowledgement` is the dispatch-status feedback path, while `loop follow-up completion` is the later `complete_dispatched_follow_up(...)` event. Accepting dispatch does not clear pending follow-up state or advance loop continuation as if the prompt already finished.

**Rationale:** Current embedded flow calls `complete_follow_up(...)` immediately after trying to queue the prompt, which collapses "prompt was enqueued" and "prompt finished" into one state transition. That hides failures and makes loop progress look complete before the follow-up prompt actually runs.

**Alternatives considered:**
- **Keep one `LoopFollowUpCompleted` step and reinterpret it loosely:** Rejected because the ambiguity is exactly the bug-prone seam.
- **Track dispatch state only in shell locals:** Rejected because the controller/core would still not own the lifecycle contract.

### 3. Keep correlation identities core-owned and fail closed on mismatch or reordering

**Choice:** `clankers-core` continues to mint the correlation identities for prompt work and follow-up work. The migrated slice keeps one unambiguous pending namespace across prompt, follow-up, and tool-filter work, with at most one pending prompt, one pending follow-up, and one pending tool-filter rebuild live at a time; shell feedback must echo the matching identity and stage. Wrong identity, wrong stage, duplicate feedback, or out-of-order feedback is rejected without mutating previously valid state. Controller shells keep surfacing those failures through the existing error `DaemonEvent::SystemMessage` category, while embedded shells keep surfacing them through the existing standalone TUI `App::push_system(..., true)` path instead of inventing daemon-only event types. Failed follow-up completion keeps the same shell-specific error-surfacing rule.

**Rationale:** The spec for this capability already depends on explicit correlation ownership and rejection behavior. Extending embedded prompt lifecycle without naming how identities are sourced and rejected would reopen the same ambiguity the first slice just closed.

**Alternatives considered:**
- **Mint runtime-local dispatch IDs in `event_loop_runner`:** Rejected because core ownership of correlation would be lost.
- **Permit silent no-op handling for stale or mismatched feedback:** Rejected because typed rejection is part of the no-std functional-core contract.

### 4. Keep queued prompt and runtime transport facts shell-native, but pass only the minimum plain data into `clankers-core`

**Choice:** The migrated prompt-lifecycle slice accepts plain-data inputs such as `queued_prompt_present`, prompt completion outcome, and follow-up dispatch result, but it does not accept Tokio senders, `TaskResult`, TUI widgets, or queued prompt text itself. `clankers-core::CoreState` remains the home for the post-prompt facts that persist across transitions: active loop state, pending follow-up state, auto-test enabled state, auto-test command presence, and auto-test in-progress state. The shell keeps the actual queued prompt text and replays it only when the core-selected plan says queued replay wins.

**Rationale:** The deterministic rule only needs to know whether queued replay is available, not the text payload itself. Keeping the payload shell-side narrows the public transition shape while still moving the precedence decision into `clankers-core`.

**Alternatives considered:**
- **Pass queued prompt text through the core boundary:** Rejected because the current slice only needs presence and would widen the public transition surface for no deterministic gain.
- **Move queued prompt handling into `clankers-core` with shell-native wrapper types:** Rejected because it leaks runtime concerns across the boundary.
- **Leave queued prompt outside the migrated slice entirely:** Rejected because precedence between queued prompt replay and controller follow-up is the central deterministic decision this change exists to capture.

### 5. Concentrate migrated shell execution in `crates/clankers-controller/src/core_effects.rs`

**Choice:** Controller-owned shell execution for this slice continues to funnel through `core_effects.rs`, extended as needed for post-prompt planning and follow-up dispatch handling. `command.rs`, `auto_test.rs`, and embedded runtime code translate inputs and call that executor rather than each growing their own follow-up policy branches. The same controller-owned adapters remain the only place where `clankers-core` types are translated into runtime-facing calls; `clankers-agent` runtime and public APIs stay shell-native for this slice.

**Rationale:** The previous slice already identified `core_effects.rs` as the shared shell seam. Reusing it prevents the new slice from re-fragmenting effect handling across controller and embedded runtime paths.

**Alternatives considered:**
- **Add new local effect executors in `event_loop_runner` or `auto_test.rs`:** Rejected because it recreates duplicated shell semantics.
- **Inline execution back into `command.rs` and `auto_test.rs`:** Rejected because review would again need to reason about several policy-adjacent execution paths.

### 6. Keep verification aligned with the full `no-std-functional-core` rail set

**Choice:** Acceptance for this slice extends the existing verification bundle rather than inventing a side path. The design explicitly requires:
- `cargo check -p clankers-core --no-default-features --target thumbv7em-none-eabi`
- `scripts/check-clankers-core-boundary.sh`
- `scripts/check-clankers-core-surface.sh`
- a determinism replay rail for the migrated prompt-lifecycle slice
- reducer coverage for busy gating, prompt start, prompt completion, follow-up dispatch feedback, loop continuation, `StartLoop` / `StopLoop`, pending-follow-up loop-control rejection, `notify_prompt_done()`, `check_post_prompt()`, thinking changes, tool-filter changes, queued-prompt precedence, loop-over-auto-test precedence, mismatched feedback rejection, duplicate or wrong-stage feedback rejection, and out-of-order runtime-result rejection
- controller parity coverage in `crates/clankers-controller/src/{command.rs,auto_test.rs,core_effects.rs}`
- queued user-prompt replay winning over controller-generated follow-up when both are eligible, the simultaneous-eligibility case where loop continuation outranks auto-test, prompt-completion feedback, follow-up dispatch feedback, filtered-tool rebuild application, failed loop follow-up completion, mismatched, wrong-stage, or out-of-order lifecycle feedback surfacing through error `DaemonEvent::SystemMessage`, one `SessionController::handle_command(SessionCommand::Prompt { .. })` seam, and explicit `SetDisabledTools`, `CycleThinkingLevel`, and `StartLoop` / `StopLoop` regressions
- embedded runtime seam coverage in `src/modes/event_loop_runner/mod.rs`, explicitly asserting queued-prompt precedence, controller-owned next-action dispatch, explicit follow-up-dispatch rejection handling, mismatched, wrong-stage, or out-of-order rejection surfacing through `App::push_system(..., true)`, failed-prompt queued-replay behavior, and no premature loop-follow-up completion
- agent parity coverage in `crates/clankers-agent/src/turn/mod.rs` plus one migrated-slice regression in `crates/clankers-agent/src/lib.rs`, explicitly asserting shell-visible prompt or prompt-completion behavior stays aligned and lifecycle-feedback rejection stays on the shared shell-parity path instead of agent-local reinterpretation
- an anti-fork review over controller, agent, and runtime seams.

**Rationale:** The delta spec makes these rails acceptance-blocking. Design must name them explicitly so implementation and review do not quietly narrow verification to runtime tests only.

**Alternatives considered:**
- **Only expand reducer tests:** Rejected because runtime glue is the actual seam being retired.
- **Only do runtime seam tests:** Rejected because the capability already promises compile, boundary, surface, determinism, and parity rails together.
- **Only do manual review:** Rejected because this slice is subtle and regression-prone.

## Risks / Trade-offs

- **[Scope creep into full turn-loop rewrite]** → Keep the change limited to post-prompt ordering and follow-up lifecycle; do not migrate provider streaming or tool execution.
- **[Spec drift from current follow-up naming]** → Document lifecycle stages in design and tests first, then rename APIs only if it materially improves clarity.
- **[Queued prompt precedence semantics surprise users]** → Preserve current embedded behavior as the baseline and pin it with focused parity tests before moving code.
- **[Too much logic lands in controller shell code instead of `clankers-core`]** → Any deterministic rule that does not require direct I/O must land in `clankers-core`; controller code may translate and execute, but not permanently own a second policy layer.
- **[Runtime shells still mutate core-visible state directly]** → Anti-fork review must inspect `event_loop_runner`, `auto_test.rs`, and `core_effects.rs` together and reject any new ambient-state mutation path.

## Migration Plan

1. Add characterization tests around current embedded prompt completion and queued prompt replay behavior.
2. Introduce explicit data types or transition helpers for post-prompt planning and follow-up dispatch/result handling.
3. Rewire `event_loop_runner` to request and execute a controller-selected post-prompt plan instead of branching locally.
4. Split follow-up dispatch acknowledgement from later prompt completion and route both through explicit controller/core feedback.
5. Extend the existing no-std compile, boundary, surface, determinism, controller parity, embedded runtime parity, agent parity, and anti-fork verification rails.
6. Keep rollback simple: revert the new planning/dispatch path and fall back to existing embedded runtime logic if parity breaks during implementation.

## Open Questions

- None blocking. Rename the current `complete_follow_up(...)` seam to a dedicated dispatch-acknowledgement API during implementation if that improves clarity, but the design contract is already fixed: dispatch acknowledgement and later prompt completion are separate adapter entrypoints.
