# Batch Trajectory Runner Specification

## Purpose

Defines the foreground local batch runner that processes JSONL prompt jobs with bounded concurrency and exports structured trajectories for evaluation, review, or training preparation.

## Requirements

### Requirement: Batch Processing and Trajectory Export Capability [r[batch-trajectory-runner.capability]]
The system MUST provide a documented foreground local batch runner that reads many prompt jobs from a local input file with bounded concurrency and exports structured local trajectories for evaluation or training preparation.

#### Scenario: Primary path succeeds [r[batch-trajectory-runner.scenario.primary-path]]
- GIVEN the user invokes `clankers batch run` with a valid local JSONL prompt file and output directory
- WHEN the runner processes the jobs within the configured concurrency limit
- THEN clankers writes structured result metadata and trajectory output files and returns a user-visible run summary

#### Scenario: Unsupported configuration is explicit [r[batch-trajectory-runner.scenario.unsupported-config]]
- GIVEN the user invokes a remote dataset, detached daemon execution, unsupported export target, or unbounded concurrency
- WHEN clankers cannot safely proceed in the first-pass implementation
- THEN clankers MUST return an actionable unsupported error instead of silently falling back or dropping work

### Requirement: Batch Processing and Trajectory Export Session Observability [r[batch-trajectory-runner.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[batch-trajectory-runner.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: Batch Processing and Trajectory Export Verification [r[batch-trajectory-runner.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[batch-trajectory-runner.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure
