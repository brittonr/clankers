Task-ID: V4f
Covers: embeddable-agent-engine.explicit-adapter-composition, embeddable-agent-engine.adapter-held-prompt-correlation
Artifact-Type: validation-evidence

# V4f controller composition evidence

## Test paths

- `crates/clankers-controller/src/core_engine_composition.rs` unit tests:
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

## Commands

- `cargo test -p clankers-controller --lib core_engine_composition`: PASS (10 tests).

## Result

The pure `pub(crate)` controller composition seam remains testable without daemon/TUI/provider/tool execution, plans carry `CoreEffectId`, accepted prompt kind, and engine prompt seed outside `EngineState`, terminal outcomes map back to the right `CoreInput`, stale feedback is rejected by `clankers-core`, and wrong-reducer feedback is rejected.
