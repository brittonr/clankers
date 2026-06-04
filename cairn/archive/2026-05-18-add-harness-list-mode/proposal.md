## Why

The harness now emits durable run-scoped receipts, but operators and agents still have to read `scripts/test-harness.sh` to discover supported modes, selectors, receipt paths, and controlling environment variables.

## What Changes

- Add a cheap `list` mode to print the canonical harness modes, selectors, environment toggles, and receipt locations.
- Regression-test the `list` output from Rust so discoverability does not drift from the harness contract.

## Impact

- **Files**: `scripts/test-harness.sh`, `tests/test_harness_contract.rs`, `openspec/specs/test-harness/spec.md`.
- **APIs**: Adds `./scripts/test-harness.sh list` as an operator-facing CLI surface.
- **Testing**: Focused cargo test for `test_harness_contract`, OpenSpec validation, formatting, and whitespace checks.
