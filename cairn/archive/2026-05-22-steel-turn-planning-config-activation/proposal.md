# Steel Turn Planning Config Activation Proposal

## Summary

Activate the already-wired `steel.host.plan_turn` adapter from reviewed configuration instead of leaving every real `TurnConfig` construction hard-coded to `None`.

This change keeps the existing seams intact: Nickel declares the profile/script/budget/rollout contract, Rust loads and validates that data, UCAN/session grants remain the runtime authority boundary, Steel Scheme only returns typed planning data, and Rust still owns provider calls, tool execution, fallback, receipts, and blocking.

## Motivation

`steel-agent-turn-wiring` connected the planner seam to the real agent turn path, but normal runtime call sites still pass `steel_turn_planning: None`. That means the dogfood seam exists but cannot be selected by reviewed runtime config. The next slice should make the seam opt-in from stable settings/profile data and prove disabled/comparison/default modes are selected without granting Steel ambient authority.

## Non-goals

- Do not make Steel globally default without an explicit reviewed profile selecting default mode.
- Do not expose filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, or mutation authority to Steel.
- Do not let scripts self-select rollout mode, host functions, budgets, fallback, or capabilities.
- Do not replace Rust provider/tool execution or provider/router request ownership.
- Do not implement unrelated Wasm or self-mutation expansion.

## Proposed change

Add a configuration activation layer that maps reviewed settings/profile data into `AgentTurnSteelPlanningConfig` for real agent turns.

The implementation should:

1. Add a stable settings surface for Steel turn planning activation.
2. Load/validate the Nickel-exported orchestration profile and script binding through Rust-owned code.
3. Thread the resulting optional `AgentTurnSteelPlanningConfig` into both normal and orchestrated turn `TurnConfig` construction.
4. Keep the default disabled unless config explicitly enables a profile.
5. Preserve comparison/default/fallback behavior from the existing adapter and runtime DTOs.
6. Add deterministic checker/docs/tests proving the seam is config-selected and fail-closed.

## Expected outcome

A user or project can opt into Steel turn planning by declaring a reviewed profile/config, and ordinary agent turns will use that profile in disabled/comparison/default mode while receipts prove Rust authorization and fallback boundaries remain intact.
