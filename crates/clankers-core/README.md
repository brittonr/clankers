# clankers-core

`clankers-core` is the portable `#![no_std]` + `alloc` functional core for the first extracted Clankers orchestration slice.

## Boundary

Put logic in `clankers-core` when all inputs can be supplied as plain data and the result can be expressed as next state plus ordered `CoreEffect` values.

Keep logic in shell adapters when the code must perform or translate runtime work:

- provider calls
- tool inventory rebuilding and tool execution
- daemon/TUI event emission
- loop-engine mutation
- filesystem, network, process, clock, or Tokio work

Rule of thumb: if the behavior can be tested with `CoreState + CoreInput -> CoreOutcome`, it belongs in `clankers-core`. If it creates shell-native values or performs I/O, it belongs in `clankers-controller`, `clankers-agent`, or mode/runtime adapters.

## Follow-up Seams

Verification of the first slice found a few remaining seams that should stay adapter-only until a later extraction moves more policy into the core:

- `crates/clankers-controller/src/core_effects.rs` is now the shared controller shell executor for the migrated slice. Future extractions should extend that effect-execution seam instead of reintroducing per-command or per-auto-test policy branches.
- `src/modes/event_loop_runner/mod.rs` remains embedded-mode runtime plumbing. Future prompt-lifecycle work should keep that file as TUI/runtime wiring that forwards controller-owned prompt start/completion contracts instead of minting local prompt or loop policy.
- `crates/clankers-agent/src/lib.rs`, `crates/clankers-agent/src/turn/mod.rs`, and `crates/clankers-agent/src/turn/execution.rs` intentionally remain shells: they apply controller-owned thinking/tool-filter effects and execute the controller-provided tool inventory per call. Future extractions should move only deterministic policy into the core, not the actual tool execution or provider streaming paths.
