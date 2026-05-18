## Why

Clankers has many focused tests, fake-provider E2E paths, and receipt checks, but the highest-risk regressions span provider request construction, session correlation, tool-call ordering, transcript mutation, event streaming, daemon/controller adapters, and harness receipts. Those paths are expensive or flaky to validate through live providers and too cross-cutting for isolated unit tests.

A deterministic turn replay harness gives Clankers a cheap, credential-free acceptance rail: fixed inputs, scripted model/tool feedback, normalized outputs, and byte-stable receipts that can be rerun locally and in CI.

## What Changes

- Add a deterministic agent-turn replay capability with scripted provider responses, scripted tool results, normalized event/transcript output, and BLAKE3-bound receipts.
- Require replay-equivalence tests that run the same fixture twice and prove normalized outputs are identical.
- Require request-shape assertions for session identifiers, message ordering, tool schemas, and continuation after tool results.
- Add a harness profile for deterministic replay and cover it in the harness discoverability/receipt contract.

## Capabilities

### New Capabilities
- `deterministic-agent-testing`: Credential-free deterministic turn replay fixtures and receipts.

### Modified Capabilities
- `test-harness`: Expose and document a deterministic replay profile.

## Impact

- **Files**: likely `tests/deterministic_turn_replay.rs`, `tests/fixtures/deterministic_turn/**`, `scripts/test-harness.sh`, and small test-helper modules in `crates/clankers-controller` / `crates/clankers-agent` as needed.
- **APIs**: test-only fixture/replay helpers; no user-facing runtime API required beyond the harness profile.
- **Dependencies**: avoid live network/OAuth; prefer existing BLAKE3/serde/tempfile dependencies where possible.
- **Testing**: focused deterministic replay tests, harness dry-run contract coverage, OpenSpec validation, formatting, and diff checks.
