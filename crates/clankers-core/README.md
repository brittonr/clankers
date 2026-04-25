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
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` is the standing source-inventory rail for these seams. It parses non-test Rust items directly, then asserts the agent runtime files stay free of `clankers_core`, `src/modes/event_loop_runner/mod.rs` stays free of reducer/effect-interpretation types, `crates/clankers-controller/src/core_effects.rs` remains the only prompt-lifecycle `CoreEffect` interpreter across controller runtime sources, controller `CoreInput` translation stays centralized in `crates/clankers-controller/src/{command.rs,auto_test.rs}`, controller event/output translation stays centralized in `crates/clankers-controller/src/convert.rs` with `src/event_processing.rs` limited to calling the shared converter rather than rebuilding translated protocol events inline, and transport/client framing files stay limited to protocol I/O while pure wire construction (`Handshake`, `SessionInfo`, `SessionSummary`, `DaemonStatus`) stays centralized in `crates/clankers-controller/src/transport_convert.rs`.

## Reducer Ownership Matrix

`clankers-core` owns deterministic lifecycle/control policy that can be expressed as `CoreState + CoreInput -> CoreOutcome`:

- prompt lifecycle acceptance, completion, busy state, and queued prompt replay
- loop and auto-test follow-up dispatch/completion sequencing
- thinking-level changes
- disabled-tool filter state and rebuild effects
- cancellation before accepted prompt/follow-up work reaches `clankers-engine`

`clankers-engine` owns model/tool turn policy only after an adapter submits an accepted prompt or follow-up:

- model request correlation and model completion/failure feedback
- tool-call planning, pending tool-call state, and tool result ingestion
- retry scheduling and retry-ready feedback
- model-request continuation budget
- cancellation during model/tool/retry phases
- terminal turn outcomes after engine-owned work starts

Composition belongs in controller/agent adapters. Core effect IDs remain core-owned correlation values; adapters may hold and echo them back, but `clankers-engine` must not store `CoreState`, `CoreEffectId`, or other core lifecycle types.
