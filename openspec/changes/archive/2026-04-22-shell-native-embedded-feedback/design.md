## Context

`clankers-core` owns the deterministic prompt-lifecycle policy, but `src/modes/event_loop_runner/mod.rs` still constructs `clankers_core` feedback types when it acknowledges follow-up dispatch or reports prompt completion. That is a small but real boundary leak: the root runtime adapter knows about reducer correlation and failure payload shapes that should stay controller-owned.

This slice should remove that leak without moving more policy into the runtime shell. The controller already owns pending prompt state, post-prompt planning, and the conversion between shell events and core inputs, so the clean seam is to make controller APIs and `PostPromptAction` shell-native while preserving internal core translation in `crates/clankers-controller`.

## Goals / Non-Goals

**Goals:**
- Remove non-test `clankers_core` feedback construction from `src/modes/event_loop_runner/mod.rs`
- Keep correlation identity and status mapping inside `crates/clankers-controller`
- Preserve embedded follow-up behavior, queued-prompt precedence, loop advancement after successful follow-up completion, rejection unchanged-state behavior, and failure surfacing
- Strengthen FCIS rails so root embedded runtime code stays free of direct core feedback types

**Non-Goals:**
- Changing reducer behavior or correlation semantics in `clankers-core`
- Reworking daemon-mode controller command handling beyond the adapter seam needed here
- Moving interactive shell work, plugin dispatch, or prompt sending into the controller or core

## Decisions

### 1. Add controller-owned shell-native feedback types

**Choice:** Introduce a controller-owned pending-work identity type plus shell-native completion and dispatch-result adapters in `crates/clankers-controller`.

**Rationale:** The root runtime only needs to say "this pending work succeeded/failed" or "dispatch was accepted/rejected" with a string message on failure. It should not know the concrete `clankers_core` enums that carry those facts internally.

**Alternative considered:** Keep using raw `clankers_core` types in `event_loop_runner` and rely on FCIS rails alone. Rejected because it leaves the boundary leak in production code and keeps future extraction work coupled to core internals.

### 2. Keep core conversion in controller adapters

**Choice:** `SessionController` remains the only place that converts shell-native feedback into `clankers_core::{PromptCompleted, LoopFollowUpCompleted, FollowUpDispatchAcknowledged}` inputs, including both `PostPromptAction::ContinueLoop` and `PostPromptAction::RunAutoTest` paths owned by `crates/clankers-controller/src/auto_test.rs`.

**Rationale:** This preserves the existing FCIS pattern: functional core owns deterministic state transitions, controller owns translation plus effect execution, runtime shells only forward shell-native facts. It also keeps the `RunAutoTest` path on the same controller-owned pending-work identity seam as loop continuation instead of letting one branch drift.

**Alternative considered:** Add a second translation seam in `src/modes/event_loop_runner/mod.rs`. Rejected because it would create another location that knows reducer status and correlation details.

### 3. Verify both loop and auto-test follow-up behavior explicitly

**Choice:** Focused tests will assert three concrete behaviors through the shell-native adapters: accepted dispatch plus successful follow-up completion advances loop state only after the follow-up prompt finishes; rejected dispatch leaves state unchanged and surfaces the existing error path; and failed prompt/follow-up completion preserves queued-prompt replay and loop-failure/error surfacing behavior.

**Rationale:** The slice is small, but it touches both loop-continuation and auto-test follow-up flows. Naming the exact checks up front keeps `tests/embedded_controller.rs`, `src/modes/event_loop_runner/mod.rs`, and `crates/clankers-controller/src/auto_test.rs` aligned with the behavior-preservation contract instead of relying on broad “parity” language.

**Alternative considered:** Depend on broad controller nextest coverage without seam-specific assertions. Rejected because this boundary leak is easy to reintroduce through one branch while the other stays green.

### 4. Upgrade the FCIS boundary rail to ban all runtime `clankers_core` paths in `event_loop_runner`

**Choice:** Once the controller-owned adapters land, strengthen `crates/clankers-controller/tests/fcis_shell_boundaries.rs` to treat any non-test `clankers_core` path in `src/modes/event_loop_runner/mod.rs` as a regression.

**Rationale:** The current rail only blocks specific `Core*` segments. This slice removes the remaining direct runtime core references, so the stronger rail becomes both accurate and easier to reason about.

**Alternative considered:** Keep the narrower allowlist. Rejected because it would not catch future leakage through `CompletionStatus`, `CoreFailure`, or `FollowUpDispatchStatus`.

## Risks / Trade-offs

- **Public/controller API churn** → Mitigate by keeping the new shell-native types narrow and local to this embedded-feedback seam.
- **Behavior drift during translation rewrite** → Mitigate with focused embedded/runtime tests that cover accepted dispatch, rejected dispatch, successful follow-up completion, failed prompt/follow-up completion, loop advancement timing, and unchanged-state rejection before broad validation.
- **Over-wrapping simple IDs** → Mitigate by using one controller-owned pending-work identity type instead of several overlapping wrappers.

## Migration Plan

1. Add controller-owned shell-native feedback types and conversion helpers.
2. Rewire `PostPromptAction::{ContinueLoop, RunAutoTest}`, pending follow-up ID lookup, and embedded runtime call sites to the new types, keeping `crates/clankers-controller/src/auto_test.rs` on the same seam.
3. Update focused embedded/runtime tests and FCIS boundary rails with explicit checks for loop advancement timing, rejection unchanged-state behavior, and preserved error surfacing.
4. Rerun focused controller tests and the full `verify-no-std-functional-core` bundle.

## Open Questions

- None expected for this slice; if extra shell callers appear during implementation, they should also use the controller-owned types rather than reintroducing raw `clankers_core` status construction.
