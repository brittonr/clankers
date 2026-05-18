## ADDED Requirements

### Requirement: Harness Receipt Contract

The test harness SHALL emit machine-readable and human-readable receipts whose structure is regression-tested without running expensive readiness gates.

#### Scenario: Dry-run receipts are valid and consistent

- GIVEN a representative `scripts/test-harness.sh` mode running with dry-run enabled
- WHEN the harness exits successfully
- THEN `results.json`, `summary.md`, and `junit.xml` SHALL exist
- AND the JSON counts SHALL match the emitted step statuses
- AND the summary and JUnit output SHALL reference every emitted step

#### Scenario: Dry-run mode selection is covered

- GIVEN representative harness modes for pure, E2E, VM, and flake readiness paths
- WHEN the dry-run contract test runs those modes
- THEN each mode SHALL select its documented command steps without executing expensive checks
