Evidence-ID: turn-loop-audit
Artifact-Type: investigation-note
Task-ID: R1
Covers: r[steel-core-agent-loop.executor-selection], r[steel-core-agent-loop.no-ambient-authority]
Created: 2026-05-30
Status: complete

# Turn Loop Audit

## Findings

- `crates/clankers-agent/src/turn/mod.rs::run_turn_loop` already called `plan_agent_turn` and emitted a Steel planning receipt.
- The planning result included `AgentTurnExecutionPlanner::SteelScheme`, `RustNative`, and `Blocked`, but only `Blocked` affected control flow.
- Non-blocked turns always flowed into the existing Rust `run_engine_turn` path, so authorized default Steel planning did not select an execution path.
- `crates/clankers-engine-host/src/lib.rs::run_engine_turn` remains the reducer-backed host-effect runner used by the shell.
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` contains the static FCIS rail for keeping reducer/runner ownership centralized.

## Conclusion

The safe first executable slice is to add a narrow Steel-selected execution seam in the agent turn module, branch to it only for authorized `SteelScheme` plans, and keep concrete provider/tool host effects delegated to the existing Rust engine runner.
