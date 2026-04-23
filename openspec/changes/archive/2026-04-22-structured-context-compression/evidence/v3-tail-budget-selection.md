Artifact-Type: verification-note
Evidence-ID: v3-tail-budget-selection
Task-ID: V3
Covers: structured.compaction.tail.budgetprotection.contextwindowadapts, structured.compaction.tail.budgetprotection.shortrecentmessages, structured.compaction.tail.budgetprotection

## Summary
Deterministic unit coverage for token-budget tail protection behavior, including the default 40% tail-budget configuration.

## Evidence
- Source under test: `crates/clankers-agent/src/compaction.rs`
- Verification rail covers tail-budget adaptation, short-message preservation, and default tail-budget configuration.

## Checks
- Tail protection scales with larger context windows.
- Short recent messages preserve more tail messages than fixed-count protection.
- Default `tail_budget_fraction` remains 0.40 when unset.
