## Why

Steel turn planning is implemented, documented, and wired into the real agent turn path, but ordinary Clankers sessions still default to Rust-native planning because missing `steelTurnPlanning` settings disable the seam. The product direction is now to make the reviewed `steel.host.plan_turn` path the default planner for supported real turns while preserving Rust-owned authority and an explicit operator kill switch.

Defaulting Steel should not grant ambient authority. Steel remains a constrained embedded planner that emits typed plan data and redacted receipts. Rust continues to own provider calls, tool execution, fallback/block decisions, session state, mutation, and all host effects.

## What Changes

- Change the default turn-planning behavior so missing `steelTurnPlanning` settings activate the bundled reviewed `steel.host.plan_turn` profile and script.
- Preserve `steelTurnPlanning.enabled = false` as the explicit opt-out / kill switch that keeps Rust-native planning and emits no Steel-authorship claim.
- Require default activation to be hash-bound to checked-in policy/script material, constrained to the current session turn resource, and receipt-backed.
- Keep `steel_eval` unchanged: the agent-visible pure eval tool remains separate from turn planning.

## Impact

- **Files likely affected**: `crates/clankers-config/src/settings.rs`, `crates/clankers-agent/src/turn/steel_planning.rs`, `crates/clankers-agent/src/lib.rs`, `policy/steel-default-orchestration/*`, Steel docs/checkers, and tests under `crates/clankers-agent` / `tests/embedded_controller.rs`.
- **Testing**: focused Steel config/default tests, real turn smoke, Steel checker receipts, Cairn gates/validation, docs build, and diff checks.
- **Non-goals**: no new Steel host functions, no mutation-capable default, no provider/tool execution inside Steel, no OS/process sandbox claim, no removal of Rust-native fallback or operator opt-out.
