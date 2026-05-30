Evidence-ID: focused-validation
Artifact-Type: validation-log
Task-ID: V1
Covers: r[steel-execute-turn-host-call.runtime.allowed], r[steel-execute-turn-host-call.runtime.denied], r[steel-execute-turn-host-call.runtime.malformed], r[steel-execute-turn-host-call.receipts.allowed], r[steel-execute-turn-host-call.receipts.denied], r[steel-execute-turn-host-call.verification.checker], r[steel-execute-turn-host-call.verification.real-denial]
Status: pass

# Focused Validation

## Commands and Results

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-runtime execute_turn --lib -- --nocapture`
  - Result: pass; 3 tests passed.
  - Covered tests: `execute_turn_authority_requires_execution_capability_and_ucan`, `execute_turn_authority_denies_missing_ucan_or_disabled_action_before_host_runner`, `execute_turn_host_call_rejects_malformed_payload_before_authorized_status`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-agent steel --lib -- --nocapture`
  - Result: pass; 24 tests passed.
  - Covered real turn-loop tests include `run_turn_loop_uses_steel_selected_executor_when_default_planner_authorizes` and `run_turn_loop_emits_steel_plan_turn_receipt_when_configured`.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test --test embedded_controller steel_runtime_smoke -- --nocapture`
  - Result: pass; 6 tests passed.
  - Covered smoke includes provider-call denial with provider count `0` and daemon-visible host-call denial fields.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-execute-turn-host-call.rs`
  - Result: pass; wrote `target/steel-execute-turn-host-call/receipt.json`.

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

The focused tests and checkers assert execution receipts include safe host-call metadata (`host_call_status`, `host_call_reason`, `host_call_outcome`, `host_call_payload`, `host_call_receipt_hash`) plus authority metadata. They assert raw prompt text, Steel script source, provider payloads, tool bodies, credentials, and UCAN proofs are absent from daemon-visible receipts.
