Artifact-Type: verification-note
Evidence-ID: v2-manual-compact-shared-path
Task-ID: V2
Covers: structured.compaction.pruning.prepass.manualcompactsharedpath

## Summary
Deterministic regression coverage for `/compact` shared pruning path behavior in both standalone and daemon-backed session flows.

## Evidence
- Source under test: `src/modes/event_loop_runner/key_handler.rs`
- Verification rail covers shared pruning mutation of old tool results and intact recent tail tool results across standalone and daemon-backed `/compact` flows.

## Checks
- Standalone `/compact` uses shared pruning path.
- Daemon-backed `/compact` uses shared pruning path.
- Older tool results are pruned through shared path.
- Recent tail tool results remain intact.
