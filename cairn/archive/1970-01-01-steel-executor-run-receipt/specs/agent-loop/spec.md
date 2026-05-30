# Agent Loop Steel Executor Run Receipt Delta

## Purpose

Extends the agent-loop Steel executor requirements with deterministic execution-level receipt evidence from the Steel-selected adapter.

## Requirements

### Requirement: Steel-selected execution emits run receipt [r[steel-executor-run-receipt.execution-receipt]]

When an authorized default Steel plan routes a turn through the Steel-selected execution adapter, Clankers MUST emit a deterministic, redacted execution receipt proving that adapter ran and returned from the Rust-owned host runner.

#### Scenario: default execution produces adapter receipt [r[steel-executor-run-receipt.execution-receipt.default]]
- GIVEN reviewed Steel turn planning selects `executor=SteelScheme`
- WHEN the Steel-selected execution adapter finishes its host-runner call
- THEN Clankers MUST emit a `steel.host.execute_turn` receipt
- AND the receipt MUST include the executor, session hash, model label, result class, host-runner label, safe counts, and receipt hash

#### Scenario: Rust-native paths do not claim Steel execution [r[steel-executor-run-receipt.execution-receipt.rust-native]]
- GIVEN Steel planning is disabled or running in comparison mode
- WHEN the prompt executes through the Rust-native path
- THEN Clankers MUST NOT emit a Steel-selected execution receipt

### Requirement: Execution receipt remains redacted [r[steel-executor-run-receipt.redaction]]

The Steel-selected execution receipt MUST NOT include raw prompt text, provider payloads, tool bodies, credentials, UCAN proofs, raw Steel scripts, or absolute secret paths.

#### Scenario: receipt omits sensitive inputs [r[steel-executor-run-receipt.redaction.no-secrets]]
- GIVEN a prompt contains user text
- WHEN a Steel-selected execution receipt is emitted
- THEN the receipt MUST omit that raw user text
- AND it MUST contain only bounded safe metadata plus hashes
