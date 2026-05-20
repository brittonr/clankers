Artifact-Type: oracle-checkpoint
Task-ID: H1
Covers: openspec-review-gates.oracle-checkpoints, openspec-review-gates.metrics-derived-omission-prevention.safe-snapshot
Reviewed-Evidence: openspec/changes/archive/2026-05-20-roi-01-harden-openspec-gate-omission-prevention/evidence/review-metrics-snapshot.md; openspec/AGENTS.md; docs/src/reference/openspec-review-gates.md; scripts/check-openspec-review-gates.rs
Decision: Accepted. The metrics-derived scope is sanitized and limited to repeated omission classes. Human/oracle-routed closeout is not represented by prose alone: the durable guidance requires explicit H# tasks and Artifact-Type oracle-checkpoint evidence, and the checker rejects missing/prose-only oracle fixtures.
Follow-Up: Keep scripts/fixtures/openspec-review-gates, docs/src/reference/openspec-review-gates.md, openspec/AGENTS.md, and scripts/check-openspec-review-gates.rs aligned when adding new repeated omission classes.
