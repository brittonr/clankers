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

### I7 — core pre-engine cancellation reducer tests

Status: PLANNED

### I8 — controller pre-engine cancellation parity tests

Status: PLANNED

### I9 — thinking and disabled-tool ownership tests

Status: PLANNED

### V1 — positive composition sequencing

Status: PLANNED

### V2 — negative composition sequencing

Status: PLANNED

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
