# process-job-retention-gc Specification

## Purpose

Define deterministic retention, garbage collection, native log overflow, missing-log degradation, typed receipts, and NixOS integration requirements for durable Clankers process jobs.

## Requirements
### Requirement: Retention classes and eligibility [r[process-job-retention-gc.policy]]

The system MUST classify process/job metadata, logs, notification events, and tombstones into retention classes with deterministic cleanup eligibility rules.

#### Scenario: retention classes are explicit [r[process-job-retention-gc.policy.classes]]

- GIVEN a process/job record is created or updated
- WHEN retention metadata is persisted
- THEN Clankers MUST record a retention class or policy reference for metadata, native log refs, backend log refs, notification events, and tombstones
- THEN cleanup eligibility MUST be derived from configured age, size, count, status, and class rules rather than ad-hoc file scanning alone

#### Scenario: active jobs are protected [r[process-job-retention-gc.policy.active-protection]]

- GIVEN a job is running, queued, reattached, backend-unavailable, or unreconciled but not terminal
- WHEN completed-job retention or manual GC runs
- THEN Clankers MUST skip the active job metadata and logs
- THEN the GC receipt MUST report the skipped active job count or ids according to safe projection policy

### Requirement: Native log overflow and disk pressure behavior [r[process-job-retention-gc.logs]]

The system MUST bound native log growth and degrade log availability explicitly when output limits or disk errors occur.

#### Scenario: output overflow is explicit [r[process-job-retention-gc.logs.overflow]]

- GIVEN a native process emits output beyond configured max line, chunk, file, or total log limits
- WHEN Clankers stores or returns logs
- THEN it MUST truncate or rotate according to policy and record dropped/truncated counters
- THEN poll/log receipts MUST include truncation/degradation details rather than implying complete output

#### Scenario: missing log degrades query not registry [r[process-job-retention-gc.logs.missing]]

- GIVEN metadata references a native log file or backend log cursor that is missing or externally expired
- WHEN a caller lists, polls, or logs the job
- THEN Clankers MUST keep safe metadata queryable and return `log_unavailable` or equivalent typed detail for log access
- THEN it MUST NOT fail the entire registry query solely because one log reference is missing

### Requirement: Typed garbage collection receipts [r[process-job-retention-gc.receipts]]

The system MUST return machine-readable receipts for automatic or explicit process/job garbage collection.

#### Scenario: GC receipt summarizes cleanup [r[process-job-retention-gc.receipts.typed]]

- GIVEN retention cleanup runs
- WHEN Clankers returns a GC result
- THEN the receipt MUST include removed metadata count, tombstoned count, deleted native log file count, reclaimed bytes where known, released backend refs, skipped active jobs, and failures
- THEN failures MUST be typed and must not hide successful cleanup of independent eligible records

### Requirement: NixOS retention integration [r[process-job-retention-gc.nixos]]

The NixOS module MUST expose retention/log directory defaults that align with Clankers process/job GC behavior.

#### Scenario: module materializes retention defaults [r[process-job-retention-gc.nixos.integration]]

- GIVEN process/job management is enabled in the NixOS module
- WHEN the system is built
- THEN the module MUST configure state/log directories, ownership, hardening write paths, and retention defaults for metadata/log cleanup
- THEN tmpfiles/logrotate/journald integration MUST not delete active Clankers-owned native logs outside the configured policy
