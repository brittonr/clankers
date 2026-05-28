# Proposal: Thin Agent Runtime Shell

## Problem

`clankers-agent` now calls the reusable engine, but the `Agent` type still owns concrete provider, settings, database, prompt/skill, routing, hooks, process-monitoring, cost, TUI DTO, and runtime dependencies. That makes the agent crate a product shell with embedded policy rather than a small composable SDK block.

## Proposed Change

Move concrete app-edge services behind runtime/agent service ports and make `Agent` a compatibility shell that wires those ports into the reusable runtime/engine path. The public agent API can remain for Clankers shells, but turn behavior should depend on narrow host interfaces instead of concrete workspace systems.

## Impact

- **Files**: `crates/clankers-agent/src/{lib,turn,tool,context,system_prompt}.rs`, `crates/clankers-runtime`, `src/modes/common.rs`, architecture rail scripts.
- **Testing**: fake service fixtures, agent dependency budget rail, existing turn parity tests.
