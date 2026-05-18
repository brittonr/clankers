## Phase 1: Redaction policy

- [x] [serial] [covers=process-job-security-redaction.metadata.safe-preview] Define safe metadata fields, forbidden raw fields, and centralized redaction helper interfaces. ✅ completed: 2026-05-18T06:01:22Z
- [x] [parallel] [covers=process-job-security-redaction.logs.capability-gated] Define observe/list vs bounded-log vs raw-log capability classes. ✅ completed: 2026-05-18T06:01:22Z
- [x] [parallel] [covers=process-job-security-redaction.notifications.safe-excerpt] Define bounded safe excerpt policy for receipts and notifications. ✅ completed: 2026-05-18T06:01:22Z

## Phase 2: Enforcement seams

- [x] [serial] [depends:phase-1] Wire redaction through process/job persistence and receipt projection before writing redb records or returning summaries. ✅ completed: 2026-05-18T06:01:22Z
- [x] [parallel] [covers=process-job-security-redaction.metadata.no-redb-secrets] Add redb metadata tests proving raw env/header/token values are omitted or redacted. ✅ completed: 2026-05-18T06:01:22Z
- [x] [parallel] [covers=process-job-security-redaction.logs.raw-access] Add capability tests for observe-only, bounded-log, and raw-log access. ✅ completed: 2026-05-18T06:01:22Z
- [x] [parallel] [covers=process-job-security-redaction.notifications.redacted-replay] Add notification replay tests proving excerpts remain redacted after detach/reattach. ✅ completed: 2026-05-18T06:01:22Z

## Phase 3: Verification

- [x] [serial] [depends:phase-2] Add redaction fixtures covering argv, env, headers, token-like values, paths, stdout/stderr snippets, and backend refs. ✅ completed: 2026-05-18T06:01:22Z
- [x] [serial] [depends:phase-2] Run focused redaction/capability tests, `openspec validate define-process-job-security-redaction --strict --json`, and `git diff --check`. ✅ completed: 2026-05-18T06:01:22Z
