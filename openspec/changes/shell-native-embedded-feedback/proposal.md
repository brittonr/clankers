## Why

The embedded runtime still constructs `clankers_core` feedback types directly in `src/modes/event_loop_runner/mod.rs` when it acknowledges follow-up dispatch and prompt completion. That leaks core-only correlation and failure types back into a root shell adapter that should stay shell-native, which makes the next FCIS extraction step harder to police and easier to fork.

## What Changes

- Add controller-owned shell-native adapters for embedded prompt completion and follow-up dispatch/completion feedback so standalone runtime code no longer constructs `clankers_core::{CoreEffectId, CompletionStatus, CoreFailure, FollowUpDispatchStatus}` directly
- Route `PostPromptAction::{ContinueLoop, RunAutoTest}` and pending follow-up IDs through a controller-owned pending-work identity type rather than exposing raw core IDs to root runtime code
- Keep the core-to-shell conversion inside `crates/clankers-controller`, preserving existing follow-up acceptance, rejection, successful/failed follow-up completion, loop-failure surfacing, and queued-prompt behavior
- Extend FCIS boundary rails and focused embedded/runtime tests so `src/modes/event_loop_runner/mod.rs` stays free of non-test `clankers_core` references at all

## Non-Goals

- Changing reducer state-machine behavior or core correlation semantics
- Reworking daemon-only command handling outside the embedded feedback adapter seam
- Moving prompt dispatch, plugin work, clipboard/editor work, or other interactive shell effects into `clankers-core`

## Capabilities

### New Capabilities

### Modified Capabilities
- `no-std-functional-core`: add an explicit shell-native embedded-feedback boundary requirement for controller-owned pending-work IDs, prompt completion status mapping, and runtime anti-fork rails

## Impact

- `crates/clankers-controller/src/{lib.rs,auto_test.rs,core_effects.rs}`
- `src/modes/event_loop_runner/mod.rs`
- `tests/embedded_controller.rs`
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs`
- `openspec/specs/no-std-functional-core/spec.md`

## Verification

- `tests/embedded_controller.rs` will prove controller-owned pending-work identities cross both `PostPromptAction::ContinueLoop` and `PostPromptAction::RunAutoTest`, then re-enter `crates/clankers-controller/src/auto_test.rs` through controller APIs that reconvert them to core-owned correlation data without changing accepted/rejected dispatch, successful/failed follow-up completion, loop advancement, loop-failure surfacing, or queued-prompt replay behavior on either branch.
- the inline embedded-runtime tests in `src/modes/event_loop_runner/mod.rs` will prove standalone runtime code reports accepted/rejected dispatch plus successful/failed prompt completion and successful/failed follow-up completion through controller-owned shell-native adapters for both `ContinueLoop` and `RunAutoTest`, preserves queued-prompt replay rules on rejection/failure, and keeps rejection paths state-preserving.
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs` will prove `src/modes/event_loop_runner/mod.rs` holds no non-test `clankers_core` path references at all once this adapter seam lands.
