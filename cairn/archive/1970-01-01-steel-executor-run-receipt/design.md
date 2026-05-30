# Design: Steel Executor Run Receipt

## Context

`run_turn_loop` branches to `turn/steel_execution.rs::run_steel_selected_engine_turn(...)` only when Steel planning returns `AgentTurnExecutionPlanner::SteelScheme`. That adapter delegates provider/tool effects to `clankers-engine-host::run_engine_turn(...)`, preserving Rust-owned host-effect execution.

Before this change, the adapter did not emit any production receipt of its own. Tests could infer selection from planning receipts, but runtime observability lacked an execution-level receipt.

## Design

### Execution receipt context

`run_turn_loop` passes a small `SteelSelectedExecutionReceiptContext` into `run_steel_selected_engine_turn(...)` containing:

- `session_id` for hashing only,
- `model` for a sanitized model label, and
- the agent event sender for emitting a system message.

### Receipt emission

After `run_engine_turn(...)` returns, `steel_execution.rs` emits one `AgentEvent::SystemMessage`:

```text
steel.host.execute_turn receipt ... executor=SteelScheme ... status=<Completed|Rejected|TerminalFailure> ... receipt_hash=b3:...
```

The receipt payload is deterministic and hashes its own safe fields. It includes counts for observed engine events, usage observations, and adapter diagnostics, but never prompt text, provider payloads, tool bodies, credentials, UCAN proofs, raw scripts, or absolute paths.

### Tests and checks

- `run_turn_loop_uses_steel_selected_executor_when_default_planner_authorizes` now asserts the execution receipt is emitted and redacted.
- Embedded controller default-settings smoke asserts daemon-visible `steel.host.execute_turn` receipt content.
- Comparison/disabled smoke paths assert no Steel-selected execution receipt.
- Existing Steel wiring/runtime checker scripts require the execution receipt markers.

## Safety

The receipt is emitted after the existing Rust host runner returns. It does not alter provider/tool behavior or expose Steel interpreter authority.
