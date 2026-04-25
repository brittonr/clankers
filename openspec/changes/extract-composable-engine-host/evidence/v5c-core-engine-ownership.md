Task-ID: V5c
Covers: embeddable-agent-engine.core-engine-boundary-rails, embeddable-agent-engine.engine-state-active-data, embeddable-agent-engine.agent-core-type-rail
Artifact-Type: validation-evidence

# V5c core/engine ownership rail evidence

## Test paths

- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` tests:
  - `agent_runtime_files_stay_shell_native`
  - `adapter_constructor_and_feedback_inventories_stay_on_allowed_seams`
  - `core_and_engine_reducer_policy_inventories_stay_closed`
  - `clankers_engine_surface_stays_shell_native`
- `crates/clankers-engine/src/lib.rs` unit test:
  - `engine_state_fields_are_active`

## Commands

- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (33 tests).
- `cargo test -p clankers-engine --lib`: PASS (29 tests).

## Result

FCIS rails prove agent runtime files reject core lifecycle type leakage and reducer boundaries stay closed. Engine reducer tests keep the `EngineState` active-field inventory tied to reducer behavior or written justification.
