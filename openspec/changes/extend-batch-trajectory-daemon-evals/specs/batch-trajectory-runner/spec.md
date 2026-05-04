## ADDED Requirements

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
