## MODIFIED Requirements

### Requirement: Harness Receipt Contract

The test harness SHALL emit machine-readable and human-readable receipts whose structure is regression-tested without running expensive readiness gates. Each harness invocation SHALL write primary artifacts into a unique run-scoped directory and publish stable compatibility receipt paths only after the run completes. The harness SHALL expose a cheap discoverability mode that lists supported modes, selectors, environment controls, and receipt locations. The harness SHALL expose the deterministic replay rail as a named profile that includes credential-free engine replay, controller/agent shell replay, and persisted-session resume replay.

#### Scenario: Dry-run mode selection is covered

- GIVEN representative harness modes for pure, E2E, VM, deterministic engine replay, deterministic controller replay, deterministic session-resume replay, and flake readiness paths
- WHEN the dry-run contract test runs those modes
- THEN each mode SHALL select its documented command steps without executing expensive checks

#### Scenario: Harness modes are discoverable without running gates

- GIVEN an operator or agent invokes `scripts/test-harness.sh list`
- WHEN the harness prints its profile inventory
- THEN the output SHALL include supported modes, selectors, environment controls, and run-scoped plus compatibility receipt paths
- AND the profile inventory SHALL describe deterministic engine, controller, and session-resume replay coverage
- AND the profile inventory SHALL be regression-tested without executing expensive checks
