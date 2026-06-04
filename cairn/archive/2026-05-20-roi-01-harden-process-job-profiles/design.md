## Context

The durable process/job specs already cover registry persistence, backend abstraction, bounded logs, notifications, capability gates, adoption, retention, admission, lifecycle, project profiles, and the initial `process-job-profile-kit`. The code now exposes backend-neutral DTOs in `crates/clankers-runtime/src/process_jobs.rs`, docs describe profile JSON, and `scripts/check-process-job-profile-kit.rs` guards the current brick contract.

The next hardening slice should make profile-backed starts safe enough for real project reuse: deterministic profile source selection, pure resolution, typed fail-closed policy errors, safe profile identity in receipts, and drift checks that force code/docs/fixtures/specs to move together.

## Goals / Non-Goals

**Goals:**
- Preserve functional-core / imperative-shell separation for profile parsing, policy validation, identity derivation, and receipt projection.
- Make manifest discovery precedence and profile source evidence deterministic.
- Ensure profile validation rejects unsafe backend/resource/env/cwd/path cases before backend dispatch.
- Carry safe profile identity through process/job receipts and notifications.
- Extend Rust-owned drift rails and tests so advertised profile-kit behavior is backed by durable evidence.

**Non-Goals:**
- Rewriting native, pueue, or systemd backend implementations.
- Adding credential-dependent or network-dependent profile examples.
- Promoting public unattended-production readiness.
- Replacing the direct process tool start path; profile starts should reuse the same DTO/service path.

## Decisions

### 1. Treat profile resolution as pure functional core

**Choice:** Keep manifest parsing, profile lookup, policy validation, redaction, identity envelope construction, and DTO projection in pure/testable functions that accept explicit inputs.

**Rationale:** The existing profile-kit contract already says resolution is pure. Hardening should make that property harder to regress by testing with fake backend/store hooks that panic or fail if called.

**Alternative:** Resolve profiles inside the process tool handler while it has access to daemon state and backends. Rejected because ambient access makes backend dispatch and credential leaks easier to hide.

**Implementation:** Add or refine structs/functions around `ProjectProcessJobProfiles`, `ProjectProcessJobProfilePolicy`, `StartProcessJobRequest`, `ProcessJobIdentityEnvelope`, and receipt projection helpers.

### 2. Record profile source as safe receipt metadata

**Choice:** Start/list/poll/log/wait/notification/GC receipts for profile-started jobs should expose profile name, manifest schema version, profile source label/path when safe, and policy source.

**Rationale:** Operators need to distinguish direct starts from profile starts and diagnose stale config without reading raw manifests or backend logs.

**Alternative:** Only store profile data in internal metadata. Rejected because external products and agent sessions need machine-readable proof that a job came from the intended profile/policy.

**Implementation:** Add profile metadata to shared DTOs or projection adapters, bounded/redacted with existing process-job redaction helpers.

### 3. Fail closed before backend dispatch

**Choice:** Disallowed backend, malformed command shape, secret-like env key, excessive resource limit, disallowed cwd/writable path, and ambiguous manifest source must all return typed validation errors before touching native/pueue/systemd.

**Rationale:** Backend fallback or partial dispatch must not mask profile drift or policy mistakes.

**Alternative:** Let backends reject invalid work. Rejected because backend-specific errors vary and can leak or mutate before policy failure.

**Implementation:** Use fake backend dispatch counters in tests, explicit error codes, and `scripts/check-process-job-profile-kit.rs` strings/fixture checks for durable drift prevention.

## Risks / Trade-offs

**Receipt schema growth** → Keep profile fields optional for direct starts and older artifacts; add serde defaults where existing persisted records are read.

**Over-constraining paths too early** → Specify policy hooks and fail-closed behavior without forcing one final manifest location set until implementation discovers existing config conventions.

**Duplicated spec coverage** → This change modifies `durable-process-jobs` rather than creating a parallel profile spec so profile requirements stay with the durable job capability.

## Validation Plan

- `openspec validate roi-01-harden-process-job-profiles --strict --json`
- `openspec validate --all --strict --json` if unrelated legacy failures do not dominate
- Focused unit tests in `clankers-runtime` for profile source precedence, pure resolution, identity metadata, and each negative policy class
- Focused process tool tests for profile-start receipt projection and no fallback masking
- `scripts/check-process-job-profile-kit.rs`
- `cargo fmt --check`
- `git diff --check`
