# Tasks: Steel Executor Run Receipt

## Phase 0: Audit

- [x] [serial] R1. Audit the Steel-selected execution adapter and runtime smoke to identify the missing execution-level receipt seam. [covers=r[steel-executor-run-receipt.execution-receipt], r[steel-executor-run-receipt.redaction]] [evidence=evidence/execution-receipt-audit.md]

## Phase 1: Implementation

- [x] [serial] I1. Emit a deterministic redacted `steel.host.execute_turn` receipt from the Steel-selected execution adapter after the Rust host runner returns. [covers=r[steel-executor-run-receipt.execution-receipt.default], r[steel-executor-run-receipt.redaction.no-secrets]]
- [x] [serial] I2. Extend turn-loop and embedded-controller smoke tests to assert the execution receipt for default Steel execution and its absence on Rust-native paths. [covers=r[steel-executor-run-receipt.execution-receipt.default], r[steel-executor-run-receipt.execution-receipt.rust-native], r[steel-executor-run-receipt.redaction.no-secrets]]
- [x] [serial] I3. Update Steel wiring/runtime checker scripts and docs to require the execution receipt markers. [covers=r[steel-executor-run-receipt.execution-receipt], r[steel-executor-run-receipt.redaction]]

## Phase 2: Verification

- [x] [serial] V1. Run focused turn-loop tests, embedded-controller Steel runtime smoke, and Steel checker scripts. [covers=r[steel-executor-run-receipt.execution-receipt.default], r[steel-executor-run-receipt.execution-receipt.rust-native], r[steel-executor-run-receipt.redaction.no-secrets]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Run formatting/diff hygiene plus Cairn gates, sync/archive, and validation. [covers=r[steel-executor-run-receipt.execution-receipt], r[steel-executor-run-receipt.redaction]] [evidence=evidence/cairn-validation.md]
