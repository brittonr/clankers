# Tasks: Steel Core Agent Loop

## Phase 0: Audit

- [x] [serial] R1. Audit the existing turn-loop Steel planning path, engine runner seam, and FCIS boundary rail to identify where executor selection was ignored. [covers=r[steel-core-agent-loop.executor-selection], r[steel-core-agent-loop.no-ambient-authority]] [evidence=evidence/turn-loop-audit.md]

## Phase 1: Implementation

- [x] [serial] I1. Add an explicit Steel-selected execution seam and route `AgentTurnExecutionPlanner::SteelScheme` through it from `run_turn_loop`. [covers=r[steel-core-agent-loop.executor-selection.default], r[steel-core-agent-loop.no-ambient-authority.host-effects]]
- [x] [serial] I2. Preserve `RustNative` comparison/fallback behavior and keep `Blocked` results before provider/tool effects. [covers=r[steel-core-agent-loop.executor-selection.comparison], r[steel-core-agent-loop.fail-closed.before-provider]]
- [x] [serial] I3. Include the selected executor in the deterministic Steel planning receipt and update operator-facing Steel docs. [covers=r[steel-core-agent-loop.receipts.executor]]

## Phase 2: Verification

- [x] [serial] V1. Run focused `clankers-agent` turn-loop tests that prove default Steel plans use `executor=SteelScheme` and comparison plans keep `executor=RustNative`. [covers=r[steel-core-agent-loop.executor-selection.default], r[steel-core-agent-loop.executor-selection.comparison], r[steel-core-agent-loop.receipts.executor]] [evidence=evidence/focused-turn-loop-tests.md]
- [x] [serial] V2. Run FCIS shell-boundary verification and formatting/diff hygiene checks for the touched files. [covers=r[steel-core-agent-loop.no-ambient-authority.host-effects], r[steel-core-agent-loop.fail-closed.before-provider]] [evidence=evidence/static-validation.md]
- [x] [serial] V3. Run Cairn proposal/design/tasks gates and repository Cairn validation. [covers=r[steel-core-agent-loop.executor-selection], r[steel-core-agent-loop.fail-closed], r[steel-core-agent-loop.no-ambient-authority], r[steel-core-agent-loop.receipts]] [evidence=evidence/cairn-validation.md]
