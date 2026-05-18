## Why

Clankers release/readiness claims rely on `scripts/test-harness.sh` receipts, but the harness receipt shape and dry-run mode are not themselves covered by a fast nextest-owned regression. Drift in step selection, JSON summaries, Markdown summaries, or JUnit output can make later verification evidence ambiguous.

## What Changes

- Add a nextest-owned harness contract test that runs representative harness modes in dry-run mode.
- Validate `results.json`, `summary.md`, and `junit.xml` are produced and internally consistent.
- Assert representative mode step selections stay aligned with documented readiness behavior.

## Capabilities

### Modified Capabilities
- `test-harness`: Harness dry-run receipts become regression-tested by Rust/nextest.

## Impact

- **Files**: Adds an integration test under `tests/` and OpenSpec change artifacts.
- **APIs**: No public runtime API changes.
- **Dependencies**: Uses existing test dependencies.
- **Testing**: Focused `cargo test -p clankers --test test_harness_contract`, OpenSpec validation, and diff check.
