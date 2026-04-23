Artifact-Type: verification-note
Evidence-ID: v6-strategy-and-fallback
Task-ID: V6
Covers: structured.compaction.structured.summarization.triggeredsummary, structured.compaction.structured.summarization.summarycallfallback

## Summary
Deterministic verification for structured-strategy selection and summarization-call fallback behavior.

## Evidence
- Source under test: `crates/clankers-agent/src/compaction.rs`
- Verification rail covers strategy selection, fixed 30-second summarization timeout, and both failure-path and timeout-path truncation fallback behavior.

## Checks
- Structured strategy selected only when `summary_model` is configured.
- Summarization timeout constant is fixed at 30 seconds.
- Summarization-call failure falls back to truncation compaction behavior.
- Summarization-call timeout falls back to truncation compaction behavior.
