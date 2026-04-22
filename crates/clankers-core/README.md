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

- `crates/clankers-controller/src/core_effects.rs` is the shared controller shell executor for the migrated prompt-lifecycle slice. Post-prompt planning lives in `clankers-core`; controller files forward plain-data inputs into the reducer and let `core_effects.rs` own shell execution, rejection surfacing, and follow-up dispatch/completion plumbing.
- `src/modes/event_loop_runner/mod.rs` remains embedded-mode runtime plumbing. It may replay queued user prompts, send controller-selected prompts, push standalone TUI system messages through `App::push_system(..., true)`, and forward dispatch/completion feedback, but it must not re-derive queued-prompt precedence, follow-up completion, or loop/auto-test policy locally.
- Follow-up dispatch acknowledgement and later follow-up prompt completion are distinct lifecycle stages. The shell retains the originating `follow_up_effect_id` on pending prompt metadata so later completion can re-enter `clankers-core` through the correlated follow-up-completion path instead of synthesizing success from dispatch alone.
- `crates/clankers-agent/src/lib.rs`, `crates/clankers-agent/src/turn/mod.rs`, and `crates/clankers-agent/src/turn/execution.rs` intentionally remain shells: they apply controller-owned thinking/tool-filter effects and execute the controller-provided tool inventory per call. Future extractions should keep `clankers-agent` runtime/public APIs shell-native and move only deterministic policy into the core, not the actual tool execution or provider streaming paths.
