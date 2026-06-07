# Change: Shrink Root Shell Dependency Budget

## Why

The root `clankers` crate still depends on 29 internal workspace crates. Some of that is legitimate product-shell wiring, but the ownership inventory also records many convergence conditions such as provider routing, storage, prompt discovery, skill discovery, plugin/runtime wiring, display projection, and process/tool policy. The root crate remains too coupled when reusable behavior lives in `src/` instead of named owner crates or explicit adapter modules.

## What Changes

- Convert the root dependency owner receipt into a budgeted drain plan with app-edge exceptions, temporary-policy rows, and convergence targets.
- Move reusable behavior out of root modules by slice, leaving root code as CLI parsing, service assembly, adapter selection, and edge projection.
- Require every remaining root internal dependency to have a current owner category, adapter module, convergence condition, and focused validation path.
- Track budget decreases or narrower exception classes as root policy moves into owner crates.

## Impact

- **Files**: `src/runtime_services.rs`, `src/runtime_prompt.rs`, `src/agent.rs`, `src/modes/**`, `src/tools/**`, `src/commands/**`, root Cargo manifest, architecture rails, and generated ownership receipts.
- **Testing**: root module ownership rail, focused tests for each moved behavior slice, TUI/daemon/CLI smoke for affected modes, `cargo check --tests`, Cairn gates, and diff checks.
