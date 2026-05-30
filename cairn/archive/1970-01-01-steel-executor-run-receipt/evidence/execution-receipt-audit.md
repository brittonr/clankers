Evidence-ID: execution-receipt-audit
Artifact-Type: investigation-note
Task-ID: R1
Covers: r[steel-executor-run-receipt.execution-receipt], r[steel-executor-run-receipt.redaction]
Created: 2026-05-30
Status: complete

# Execution Receipt Audit

## Findings

- `run_turn_loop` branches to `run_steel_selected_engine_turn(...)` only for `AgentTurnExecutionPlanner::SteelScheme`.
- `run_steel_selected_engine_turn(...)` delegated to `run_engine_turn(...)` and returned the report without emitting production receipt evidence.
- Planning receipts already record `executor=SteelScheme`, but that is not an adapter-run receipt.
- Embedded controller smoke observes daemon-visible `DaemonEvent::SystemMessage` receipt text and is the right deterministic runtime boundary for this proof.

## Conclusion

Emit one redacted `steel.host.execute_turn` receipt from the adapter after the Rust host runner returns, then assert that receipt in both unit-level and controller-level rails.
