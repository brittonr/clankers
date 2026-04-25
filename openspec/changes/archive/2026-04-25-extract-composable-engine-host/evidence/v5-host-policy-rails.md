Task-ID: V5e
Covers: embeddable-agent-engine.host-crate-boundary-rails, embeddable-agent-engine.no-duplicated-runner-policy
Artifact-Type: validation-evidence

# V5 host policy rail evidence

## Test paths

- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` tests:
  - `engine_host_feedback_constructors_stay_in_runtime_module`
  - `engine_host_rejects_reducer_policy_source_leakage`
  - `host_crates_reject_shell_runtime_source_leakage`
  - `tool_host_rejects_engine_reducer_internal_source_leakage`

## Commands

- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (32 tests).

## Result

Host feedback constructors are restricted to `clankers-engine-host::runtime`; host crates reject shell-runtime leakage; `clankers-engine-host` rejects retry/backoff, continuation-budget, terminalization-helper, direct `EngineEvent::TurnFinished`, and direct `StopReason::{ToolUse,MaxTokens}` policy leakage outside tests.
