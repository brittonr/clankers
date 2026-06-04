## Why

The first `no-std-functional-core` slice proved the reducer/effect boundary works, but one high-risk deterministic seam still lives in embedded-mode runtime code: prompt completion and follow-up dispatch inside `src/modes/event_loop_runner/mod.rs`. That leaves standalone TUI behavior with local policy branches that can drift from daemon/controller behavior even though both paths are meant to share one controller-owned orchestration core.

## What Changes

- Move embedded prompt-lifecycle decision logic out of `src/modes/event_loop_runner/mod.rs` and into `clankers-core` transitions plus controller-owned shell execution that return explicit next actions
- Extend the `clankers-core` boundary so prompt-done outcomes, queued prompt replay, and post-prompt follow-up eligibility are represented as explicit core inputs and outputs rather than TUI-local branching, including the rule that loop continuation outranks auto-test when both are eligible for the same post-prompt step
- Add core-owned correlation identities for the new embedded prompt-lifecycle stages so follow-up dispatch acknowledgement stays distinct from later prompt completion and mismatches fail explicitly
- Tighten `crates/clankers-controller/src/core_effects.rs` into the single shell executor for the migrated slice so follow-up dispatch, rejection handling, and loop-finish behavior are interpreted once
- Keep existing thinking-level, disabled-tool, and loop-control seams explicitly in scope for carried-forward deterministic and parity regressions wherever this migrated lifecycle touches shared reducer state or shared controller execution, while still avoiding any new standalone policy branch for those commands
- Add parity rails proving embedded mode and controller/daemon mode consume the same prompt-lifecycle decisions for this slice, including explicit rejection handling for mismatched, wrong-stage, or out-of-order lifecycle feedback, preserving standalone TUI surfacing through `App::push_system(..., true)` and controller surfacing through error `DaemonEvent::SystemMessage`, plus carried-forward regressions for `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, `StartLoop`, and `StopLoop`
- Preserve shell boundaries: terminal I/O, Tokio channels, plugin dispatch, clipboard/editor work, and actual prompt sending remain imperative shell code

## Non-Goals

- Rewriting provider streaming, tool execution, or the full turn loop
- Expanding this slice into attach-mode-specific behavior beyond the shared controller contracts it already consumes
- Moving terminal, channel, plugin, or editor side effects into `clankers-core`
- Redesigning loop UX or auto-test UX beyond the lifecycle ownership changes required by this seam

## Capabilities

### New Capabilities

### Modified Capabilities
- `no-std-functional-core`: extend the future-extraction and shell-parity contract to cover embedded prompt lifecycle, queued prompt replay ordering, and controller-owned post-prompt follow-up decisions

## Impact

- `src/modes/event_loop_runner/mod.rs` — remove embedded prompt-lifecycle policy branches and keep runtime wiring only
- `crates/clankers-controller/src/{lib.rs,core_effects.rs,auto_test.rs,command.rs}` — execute shared prompt-lifecycle shell work without re-owning policy
- `crates/clankers-core/` — add pure state/input/effect coverage for the next FCIS slice
- `crates/clankers-agent/` and embedded regressions — verify no second prompt-lifecycle policy path appears in agent or TUI shells
- Validation rails and OpenSpec evidence — add focused parity, negative-case, and anti-fork verification for this slice, including the dedicated no-std compile rail, `scripts/check-clankers-core-boundary.sh`, `scripts/check-clankers-core-surface.sh`, determinism replay, reducer coverage, controller parity, embedded runtime parity, agent parity, and the final anti-fork oracle checkpoint
