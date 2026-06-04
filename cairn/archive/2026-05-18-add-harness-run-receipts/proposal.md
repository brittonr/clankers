## Why

The Clankers test harness writes `summary.md`, `results.json`, and `junit.xml` to stable paths under `target/test-harness`. Sequential or overlapping harness invocations can overwrite those files, making it easy to read a stale or different run's receipt after long-running readiness gates.

## What Changes

- Write each harness invocation into a unique per-run receipt directory.
- Publish the stable top-level receipt files only after a run completes, preserving compatibility while reducing stale-read risk.
- Include the run identifier and run directory in machine-readable and human-readable receipts.
- Extend the nextest-owned harness contract tests to assert per-run isolation.

## Capabilities

### Modified Capabilities
- `test-harness`: Harness receipts become run-scoped and identify the exact completed run they describe.

## Impact

- **Files**: `scripts/test-harness.sh`, `tests/test_harness_contract.rs`, and OpenSpec artifacts.
- **APIs**: Adds `run_id` and `run_dir` fields to harness `results.json`; stable top-level paths remain compatibility outputs after completion.
- **Dependencies**: No new dependencies.
- **Testing**: Focused harness contract test, OpenSpec validation, and diff check.
