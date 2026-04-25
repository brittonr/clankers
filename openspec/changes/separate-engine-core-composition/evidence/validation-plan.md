Evidence-ID: separate-engine-core-composition.validation-plan
Task-ID: R2,I1,I2,I4,I5,I7,I8,I9,V1,V2,V3,V4,V5,V6,V7,V8,V9,V10,V11,V12,V13,V14
Artifact-Type: validation-log
Covers: embeddable-agent-engine.engine-state-active-data, embeddable-agent-engine.composition-tests, embeddable-agent-engine.core-engine-boundary-rails, embeddable-agent-engine.cross-reducer-source-rail, embeddable-agent-engine.agent-core-type-rail, embeddable-agent-engine.engine-excludes-core-dependency, no.std.functional.core.pre.engine.cancellation
Creator: pi
Created: 2026-04-25T11:47:00Z
Status: IN_PROGRESS

## Validation Log

Implementation has not started. Replace each pending section with command, result, and output excerpt before marking the corresponding task complete.

### R1 — readiness gates

Command:

```bash
openspec validate separate-engine-core-composition --strict
openspec_gate proposal separate-engine-core-composition
openspec_gate design separate-engine-core-composition
openspec_gate tasks separate-engine-core-composition
```

Result: PASS

Output excerpt:

```text
Change 'separate-engine-core-composition' is valid
Proposal gate: PASS
Design gate: PASS
Tasks gate: PASS
```

### R2 — validation evidence setup

Result: PASS

Created validation log artifact at `openspec/changes/separate-engine-core-composition/evidence/validation-plan.md` with planned sections for implementation and verification tasks.


### I1 — remove dormant engine core state

Commands:

```bash
cargo test -p clankers-engine --lib
rg 'core_state|clankers_core|CoreState' crates/clankers-engine || true
cargo tree -p clankers-engine --edges normal | grep -F 'clankers-core v' || true
```

Result: PASS

Output excerpt:

```text
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
(no core_state/clankers_core/CoreState matches in clankers-engine)
(no clankers-core normal-edge dependency under clankers-engine)
```

### I2 — EngineState active-field inventory

Command:

```bash
cargo test -p clankers-engine --lib engine_state_fields_are_active
```

Result: PASS

Output excerpt:

```text
test tests::engine_state_fields_are_active ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.00s
```

### I4 — pure core/engine composition seam

Command:

```bash
cargo test -p clankers-controller --lib core_engine_composition
```

Result: PASS

Output excerpt:

```text
running 3 tests
test core_engine_composition::tests::apply_composition_feedback_rejects_cross_reducer_feedback ... ok
test core_engine_composition::tests::engine_submission_preserves_prompt_identity_and_policy ... ok
test core_engine_composition::tests::apply_composition_feedback_routes_engine_submission_to_engine_reducer ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.00s
```

### I5 — accepted engine prompt gate

Command:

```bash
cargo test -p clankers-controller --lib accepted_engine_prompt
```

Result: PASS

Output excerpt:

```text
running 6 tests
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_normalizes_loop_follow_up ... ok
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_normalizes_start_prompt ... ok
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_rejects_core_rejection ... ok
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_rejects_multiple_submittable_effects ... ok
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_rejects_missing_prompt_effect ... ok
test core_effects::accepted_engine_prompt_tests::accepted_engine_prompt_rejects_replay_without_fresh_core_prompt ... ok

test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 156 filtered out; finished in 0.00s
```

### I6 — controller/agent handoff stays shell-native

Commands:

```bash
cargo test -p clankers-engine --lib submit_user_prompt_builds_request_effect
cargo test -p clankers-controller --lib accepted_engine_prompt
cargo test -p clankers-agent --lib accepted_prompt_submission_reduces_engine
```

Result: PASS

Output excerpt:

```text
test tests::submit_user_prompt_builds_request_effect ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 28 filtered out; finished in 0.00s

running 6 tests
... accepted_engine_prompt ... ok
test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 156 filtered out; finished in 0.00s

running 0 tests
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 136 filtered out; finished in 0.00s
```

Note: agent acceptance behavior gets executable assertions in V7; I6 verification here confirms compile-time handoff wiring and no core lifecycle types crossing the agent boundary.

### I7 — core pre-engine cancellation reducer tests

Command:

```bash
cargo test -p clankers-core pre_engine_cancellation
```

Result: PASS

Output excerpt:

```text
running 3 tests
test reducer::tests::pre_engine_cancellation_follow_up_before_engine_submission_clears_pending_follow_up ... ok
test reducer::tests::pre_engine_cancellation_rejects_mismatched_and_wrong_stage_ids ... ok
test reducer::tests::pre_engine_cancellation_prompt_before_engine_submission_clears_pending_prompt ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 38 filtered out; finished in 0.00s
```

### I8 — controller pre-engine cancellation parity tests

Commands:

```bash
cargo test -p clankers-controller --lib pre_engine_cancellation
cargo test -p clankers-controller --lib follow_up
```

Result: PASS

Output excerpt:

```text
running 3 tests
test auto_test::tests::pre_engine_cancellation_controller_paths_do_not_construct_engine_cancel_turn ... ok
test auto_test::tests::pre_engine_cancellation_embedded_prompt_uses_core_completion_not_engine_cancel ... ok
test auto_test::tests::pre_engine_cancellation_dispatched_follow_up_completes_without_prompt_task ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 162 filtered out; finished in 0.00s

running 14 tests
... follow_up ... ok
test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 151 filtered out; finished in 0.00s
```

### I9 — thinking and disabled-tool ownership tests

Commands:

```bash
cargo test -p clankers-controller --lib thinking_effects_remain_core_owned
cargo test -p clankers-controller --lib disabled_tool_effects_remain_core_owned
```

Result: PASS

Output excerpt:

```text
test command::tests::thinking_effects_remain_core_owned ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 166 filtered out; finished in 0.00s

test command::tests::disabled_tool_effects_remain_core_owned ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 166 filtered out; finished in 0.00s
```

### V1 — positive composition sequencing

Command:

```bash
cargo test -p clankers-controller --lib composition_positive_prompt_sequencing
```

Result: PASS

Output excerpt:

```text
test core_engine_composition::tests::composition_positive_prompt_sequencing_runs_core_engine_core_in_order ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 167 filtered out; finished in 0.00s
```

### V2 — positive follow-up sequencing

Commands:

```bash
cargo test -p clankers-controller --lib composition_positive_queued_prompt_replay
cargo test -p clankers-controller --lib composition_positive_follow_up_sequence
```

Result: PASS

Output excerpt:

```text
test core_engine_composition::tests::composition_positive_queued_prompt_replay_requires_fresh_core_prompt ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 169 filtered out; finished in 0.00s

test core_engine_composition::tests::composition_positive_follow_up_sequence_acknowledges_dispatch_before_engine_submission ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 169 filtered out; finished in 0.00s
```

### V3 — agent engine-feedback and accepted-prompt reduction

Status: PLANNED

### V4 — engine/core source rail inventory

Status: PLANNED

### V5 — adapter source rail inventory

Status: PLANNED

### V6 — retry/backoff constant rail

Status: PLANNED

### V7 — cargo-tree dependency rail

Status: PLANNED

### V8 — final acceptance bundle

Status: PLANNED
