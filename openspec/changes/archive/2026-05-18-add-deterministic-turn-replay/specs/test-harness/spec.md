## MODIFIED Requirements

### Requirement: Harness Receipt Contract

The test harness SHALL emit machine-readable and human-readable receipts whose structure is regression-tested without running expensive readiness gates. Each harness invocation SHALL write primary artifacts into a unique run-scoped directory and publish stable compatibility receipt paths only after the run completes. The harness SHALL expose a cheap discoverability mode that lists supported modes, selectors, environment controls, and receipt locations. The harness SHALL expose the deterministic replay rail as a named profile once deterministic turn replay is implemented.

#### Scenario: Dry-run receipts are valid and consistent

- GIVEN a representative `scripts/test-harness.sh` mode running with dry-run enabled
- WHEN the harness exits successfully
- THEN `results.json`, `summary.md`, and `junit.xml` SHALL exist
- AND the JSON counts SHALL match the emitted step statuses
- AND the summary and JUnit output SHALL reference every emitted step

#### Scenario: Dry-run mode selection is covered

- GIVEN representative harness modes for pure, E2E, VM, deterministic replay, and flake readiness paths
- WHEN the dry-run contract test runs those modes
- THEN each mode SHALL select its documented command steps without executing expensive checks

#### Scenario: Completed runs are isolated from stable compatibility paths

- GIVEN a harness run with a unique run identifier
- WHEN the harness writes receipt artifacts
- THEN the primary artifacts SHALL be stored under `runs/<run-id>/`
- AND stable top-level compatibility artifacts SHALL identify the same `run_id` and `run_dir` after completion
- AND per-step logs SHALL be stored under the run-scoped log directory

#### Scenario: Harness modes are discoverable without running gates

- GIVEN an operator or agent invokes `scripts/test-harness.sh list`
- WHEN the harness prints its profile inventory
- THEN the output SHALL include supported modes, selectors, environment controls, and run-scoped plus compatibility receipt paths
- AND the profile inventory SHALL include the deterministic replay profile once implemented
- AND the profile inventory SHALL be regression-tested without executing expensive checks
