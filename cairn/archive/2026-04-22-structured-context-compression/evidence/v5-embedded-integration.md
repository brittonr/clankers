Artifact-Type: verification-note
Evidence-ID: v5-embedded-integration
Task-ID: V5
Covers: structured.compaction.iterative.summaryupdates.secondcompaction, structured.compaction.iterative.summaryupdates.reopenrecovery, structured.compaction.structured.summarization.triggeredsummary, structured.compaction.structured.summarization.auxmodelunavailable

## Summary
Deterministic embedded integration coverage for structured compaction prompt reuse, reopen recovery, and fallback behavior.

## Evidence
- Source under test: `tests/embedded_controller.rs`
- Verification rails cover previous-summary reuse during second compaction, restored-summary seeding after reopen, structured summary prompt inspection, summary replacement behavior, and auxiliary-summary fallback behavior.
- Complementary persistence verification for `clankers-db/tool_results` recovery lives in the controller persistence test covering stored compaction summary recovery path.

## Checks
- Captured summary prompt includes restored previous summary.
- Structured summary prompt uses structured template and conversation excerpt.
- Generated summary replaces middle messages on successful structured compaction.
- Persisted latest summary is reopened and reused during later compaction setup.
- Persisted compaction summary remains recoverable from `clankers-db/tool_results` for debugging or recovery flows.
- Auxiliary summary unavailable path falls back away from structured summary text generation path.
