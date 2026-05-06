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
- GIVEN the user invokes a remote dataset, detached remote daemon execution, unsupported export target, or unbounded concurrency
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

### Requirement: Daemon-Backed Batch Execution [r[batch.daemon-execution]]
The system MUST run bounded batch prompts through ordinary clankers session/controller paths when daemon execution is selected.

#### Scenario: Bounded daemon run [r[batch.daemon-execution.scenario.bounded-daemon-run]]
- GIVEN a JSONL batch requests daemon execution with concurrency within limits
- WHEN the run starts
- THEN clankers creates or reuses sessions and records per-job session ids and statuses

#### Scenario: Resume run [r[batch.daemon-execution.scenario.resume-run]]
- GIVEN a previous batch run has a manifest with completed and failed jobs
- WHEN resume is requested
- THEN clankers skips completed jobs and retries only eligible incomplete jobs

### Requirement: Evaluation and RL Trajectory Export [r[batch.eval-export]]
The system MUST export trajectories with enough structured evidence for evals or RL datasets while applying redaction and provenance policy.

#### Scenario: Export trajectories [r[batch.eval-export.scenario.export-trajectories]]
- GIVEN a batch run completes with prompts, responses, tool events, and scores
- WHEN export is requested
- THEN clankers writes deterministic JSONL/ShareGPT/eval records with run id, job id, model/session provenance, redaction status, and objective metrics when available
