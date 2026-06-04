Artifact-Type: verification-note
Evidence-ID: v4-handoff-framing
Task-ID: V4
Covers: structured.compaction.summary.handoffframing.modelreceivescompactedcontext

## Summary
Deterministic unit coverage for compacted-summary handoff framing.

## Evidence
- Source under test: `crates/clankers-agent/src/compaction.rs`
- Verification rail checks handoff prefix presence plus latest-user-message and do-not-re-execute guidance.

## Checks
- Compacted summaries are prefixed with handoff notice.
- Handoff notice frames summary as background reference.
- Handoff notice tells the model to respond only to the latest user message.
- Handoff notice tells the model not to re-execute work from summary.
