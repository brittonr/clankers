# Design: Steel Core Agent Loop

## Context

`run_turn_loop` evaluates optional Steel turn planning through `turn/steel_planning.rs`. The planner returns an `AgentTurnExecutionPlanner`:

- `RustNative` for disabled/comparison/fallback behavior,
- `SteelScheme` when a default-mode Steel plan is authorized, or
- `Blocked` when policy requires fail-closed behavior before effects.

Before this change, `run_turn_loop` emitted the planning receipt but always called the Rust engine runner unless the planner blocked. The selected executor was not represented in receipt text.

## Design

### Execution selection

`run_turn_loop` now records the selected planner and branches before running the engine turn:

1. Evaluate Steel planning when configured.
2. Emit the redacted planning receipt with `executor=<planner>`.
3. Return a blocked `AgentTurnResult` before provider/tool effects when the planner is `Blocked`.
4. Route `SteelScheme` through `turn/steel_execution.rs::run_steel_selected_engine_turn`.
5. Route all other cases through the existing `run_engine_turn` path.

### Steel-selected execution seam

`run_steel_selected_engine_turn` is intentionally small. It marks the Steel-selected execution path while delegating the concrete typed host effects to the existing reducer-backed engine runner and `HostAdapters`. This gives the core loop a real executor seam without granting the Steel interpreter direct provider/tool authority.

Future work can move more typed host-effect scheduling behind this seam, but the first executable slice keeps behavior-equivalent Rust host execution and makes the executor selection observable and testable.

### Receipts

`format_steel_receipt` includes `executor={:?}` alongside existing status, seam, profile, policy, plan, receipt, authorization, and fallback evidence. The receipt remains redacted and deterministic.

### Tests

Focused `clankers-agent` tests cover:

- comparison-mode receipts still selecting `executor=RustNative`,
- default-mode receipts selecting `executor=SteelScheme`, and
- blocked fallback still preventing provider requests.

The FCIS shell-boundary rail remains the static guard for reducer/runner seam ownership.

## Risks

- The first Steel-selected execution seam delegates to the existing engine runner, so it is not a full interpreter-owned loop. This is intentional: Steel selection is now real and observable, while host effects remain Rust-owned.
- Docs must avoid claiming Steel directly executes provider/tool calls.
