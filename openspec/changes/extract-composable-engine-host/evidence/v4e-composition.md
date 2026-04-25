Task-ID: V4e1,V4e2,V4e3
Covers: embeddable-agent-engine.composition-tests
Artifact-Type: validation-evidence

# V4e adapter composition evidence

## Test paths

- `crates/clankers-controller/src/core_engine_composition.rs`:
  - `engine_submission_preserves_prompt_identity_and_policy`
  - `apply_composition_feedback_routes_engine_submission_to_engine_reducer`
  - `apply_composition_feedback_rejects_cross_reducer_feedback`
  - `composition_positive_prompt_sequencing_runs_core_engine_core_in_order`
  - `composition_positive_queued_prompt_replay_requires_fresh_core_prompt`
  - `composition_positive_follow_up_sequence_acknowledges_dispatch_before_engine_submission`
  - `composition_lifecycle_failures_and_budgets_stay_explicit`
  - `composition_terminal_engine_outcome_waits_for_explicit_core_feedback`
  - `composition_follow_up_engine_failure_maps_to_loop_follow_up_completed`
  - `composition_negative_reducer_routing_rejects_wrong_targets_and_phases`
- `crates/clankers-controller/src/auto_test.rs` tests cover shell-visible prompt lifecycle, loop, auto-test, queued prompt replay, follow-up dispatch acknowledgements, duplicate completion rejection, wrong-id follow-up rejection, and pre-engine cancellation paths.
- `crates/clankers-core/src/reducer.rs` tests cover thinking, disabled-tool feedback, loop/follow-up stages, out-of-order runtime results, mismatched feedback tokens, wrong-stage feedback, and pre-engine cancellation mismatch rejection.

## Commands

- `cargo test -p clankers-controller --lib core_engine_composition`: PASS (10 tests).
- `cargo test -p clankers-controller --lib auto_test`: PASS (32 tests).
- `cargo test -p clankers-core --lib`: PASS (41 tests).

## Result

Composition tests cover core prompt acceptance/start, adapter engine submission, host-runner turn execution boundaries, core completion feedback, post-prompt follow-up evaluation, prompt lifecycle, loop and auto-test behavior, thinking/disabled-tool reducer ownership, retry/cancellation/terminal routing, out-of-order completion, mismatched effect IDs, wrong-phase engine feedback, and wrong-reducer feedback.
