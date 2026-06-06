# Change: Extract Steel Orchestration Contracts

## Why

Steel turn planning, tool substrate, and repo-evolution pack DTOs are reusable orchestration contracts, but they sit near runtime code that also owns host calls, filesystem receipts, script loading, mutation policy, and shell execution. This keeps the public runtime facade large and makes embedded hosts inherit more yellow app-edge surface than necessary.

## What Changes

- Extract Steel plan/decision/pack/source DTOs and host-call request contracts into a neutral orchestration owner.
- Keep script loading, host-call dispatch, filesystem receipt roots, Nickel export, mutation execution, and shell approval policy in runtime adapters.
- Add rails that distinguish typed orchestration contracts from executable host/runtime policy.

## Impact

- **Files**: `crates/clankers-runtime/src/steel_*.rs`, Steel policy scripts, generated runtime facade inventory, Steel repo-evolution pack checks, and turn-planning tests.
- **Testing**: Steel turn-planning fixtures, repo-evolution pack checks, runtime facade boundary rail, and aggregate embedded SDK acceptance if public labels move.
