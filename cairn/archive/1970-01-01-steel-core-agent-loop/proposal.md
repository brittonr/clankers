# Proposal: Steel Core Agent Loop

## Why

Steel turn planning already authorizes the reviewed `steel.host.plan_turn` seam and emits deterministic receipts, but the real turn loop ignored the selected executor: even an authorized default Steel plan still fell through to the Rust-native engine runner. That made "Steel default orchestration" a planning-only claim and left no runtime evidence that Steel selection affected the actual core turn path.

## What Changes

- Route authorized `AgentTurnExecutionPlanner::SteelScheme` decisions through an explicit Steel-selected execution seam in the core agent turn loop.
- Keep `RustNative` and comparison-mode behavior on the existing Rust runner.
- Keep `Blocked` decisions fail-closed before provider/tool effects.
- Include executor selection in the deterministic Steel planning receipt.
- Add focused tests and docs proving the Steel-selected executor is actually exercised while Rust still owns host effects.

## Non-Goals

- Do not grant Steel ambient filesystem, shell, network, provider, credential, daemon, TUI, native-tool, session, or mutation authority.
- Do not move provider/tool execution into a Steel interpreter.
- Do not change Steel profile/schema authority beyond the existing reviewed `steel.host.plan_turn` seam.
