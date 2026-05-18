## Context

Long-running work can emit huge logs and create durable metadata. Retention must be predictable and safe. redb should store metadata and references, not bulk logs. Native logs are files; pueue/systemd logs may be backend-owned.

## Goals / Non-Goals

**Goals:**

- Define metadata, log, notification, and tombstone retention semantics.
- Ensure GC never removes active/running jobs.
- Return typed GC receipts.
- Degrade gracefully when logs are externally removed.
- Integrate with NixOS tmpfiles/logrotate/journald where appropriate.

**Non-Goals:**

- Do not implement a general-purpose disk cleaner.
- Do not mutate backend-owned journald/pueue logs unless explicitly supported.
- Do not store bulk logs in redb.

## Decisions

### Decision 1: metadata and logs have separate retention

**Choice:** redb metadata records carry retention class and log refs. Native log files have age/size/count policies. Backend log refs are released/tombstoned, not necessarily deleted.

**Rationale:** Metadata queries and log bytes have different size/security/lifecycle concerns.

### Decision 2: GC returns typed receipts

**Choice:** GC reports records removed/tombstoned, log files deleted, bytes reclaimed, active records skipped, backend refs released, and failures.

**Rationale:** Users need proof that cleanup was safe and bounded.

### Decision 3: disk pressure degrades output before corrupting registry

**Choice:** If native log writing hits size limits or disk errors, Clankers marks log state degraded/truncated and preserves metadata/status updates where possible.

**Rationale:** Losing every handle because logs are noisy is worse than explicit truncation.

## Validation Plan

- `openspec validate define-process-job-retention-gc --strict --json`
- Unit tests for retention eligibility and active-job protection.
- Native log GC tests with temp dirs.
- Missing-log/degraded-log receipt tests.
- Nix module eval tests for retention/log directory options.
