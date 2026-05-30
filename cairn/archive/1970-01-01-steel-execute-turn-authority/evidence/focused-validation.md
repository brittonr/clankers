Evidence-ID: focused-validation
Artifact-Type: validation-log
Task-ID: V1
Covers: r[steel-execute-turn-authority.profile.separate-grants], r[steel-execute-turn-authority.pre-run.allowed], r[steel-execute-turn-authority.pre-run.denied], r[steel-execute-turn-authority.receipts.allowed], r[steel-execute-turn-authority.receipts.denied], r[steel-execute-turn-authority.verification.checker], r[steel-execute-turn-authority.verification.real-denial]
Status: pass

# Focused Validation

## Commands and Results

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-runtime execute_turn_authority --lib -- --nocapture`
  - Result: pass; 2 tests passed.
  - Covered tests: `execute_turn_authority_requires_execution_capability_and_ucan`, `execute_turn_authority_denies_missing_ucan_or_disabled_action_before_host_runner`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-agent steel --lib -- --nocapture`
  - Result: pass; 24 tests passed.
  - Covered real turn-loop tests include `run_turn_loop_uses_steel_selected_executor_when_default_planner_authorizes` and `run_turn_loop_emits_steel_plan_turn_receipt_when_configured`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test --test embedded_controller steel_runtime_smoke -- --nocapture`
  - Result: pass; 6 tests passed.
  - Covered smoke includes `steel_runtime_smoke_missing_execute_authority_fails_closed_before_provider`, which asserts provider call count stays zero on missing execution authority.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-execute-turn-authority.rs`
  - Result: pass; wrote `target/steel-execute-turn-authority/receipt.json`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-agent-turn-wiring.rs`
  - Result: pass; wrote `target/steel-agent-turn-wiring/receipt.json`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-turn-planning-runtime-smoke.rs`
  - Result: pass; wrote `target/steel-turn-planning-runtime-smoke/receipt.json`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-default-orchestration.rs`
  - Result: pass; wrote `target/steel-default-orchestration/profile-receipt.json`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-turn-planning-config-activation.rs`
  - Result: pass; wrote `target/steel-turn-planning-config-activation/receipt.json`.

## Redaction Notes

The focused tests and checkers assert execution receipts include only safe metadata: authority status/reason, required UCAN ability, required session capabilities, input hash, authority receipt hash, safe counts, and receipt hash. They assert raw prompt text and Steel script text are absent from daemon-visible receipts.
