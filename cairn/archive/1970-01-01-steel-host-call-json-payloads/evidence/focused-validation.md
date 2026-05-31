Evidence-ID: focused-validation
Artifact-Type: validation-log
Task-ID: V1
Covers: r[steel-host-call-json-payloads.plan.valid], r[steel-host-call-json-payloads.plan.legacy-denied], r[steel-host-call-json-payloads.execute.valid], r[steel-host-call-json-payloads.execute.malformed-denied], r[steel-host-call-json-payloads.receipts.hashes], r[steel-host-call-json-payloads.verification.checker]
Status: pass

# Focused Validation

## Commands and Results

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-runtime steel_orchestration --lib -- --nocapture`
  - Result: pass; 14 tests passed.
  - Covered planning JSON acceptance, legacy delimited payload fallback, execute-turn JSON acceptance, and malformed execute-turn payload denial.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test -p clankers-agent steel --lib -- --nocapture`
  - Result: pass; 24 tests passed.
  - Covered real turn-loop Steel planning/execution paths using JSON payload construction.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c cargo test --test embedded_controller steel_runtime_smoke -- --nocapture`
  - Result: pass; 6 tests passed.
  - Covered daemon-visible redacted receipts and provider-call denial behavior.

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-steel-host-call-json-payloads.rs`
  - Result: pass; wrote `target/steel-host-call-json-payloads/receipt.json`.

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

The checker and smoke tests assert receipts expose safe JSON payload metadata only: schema markers, payload validity, payload hashes, host-call status/reason, and execution authority hashes. Raw prompts, provider payloads, tool bodies, credentials, and UCAN proofs are not included.
