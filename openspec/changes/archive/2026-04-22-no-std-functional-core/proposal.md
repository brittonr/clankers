## Why

Clankers still keeps most session and turn orchestration inside `std`/Tokio-heavy code paths, so the logic that should be deterministic is hard to isolate, test, and reuse. Making the core compile as `no_std` is the first hard boundary for a real functional-core / imperative-shell split: pure state transitions and planning stay in a portable core, while filesystem, network, TUI, hooks, and provider I/O remain in thin shell adapters.

## What Changes

- Add a new `clankers-core` workspace crate that builds with `#![no_std]` and `alloc`
- Define explicit core inputs, state, and effect plans for the first session/turn orchestration slice instead of embedding decisions in `std` shell code
- Move deterministic orchestration logic into the core crate, leaving `clankers-agent`, `clankers-controller`, and mode code to execute effects and translate I/O
- Add parity and compile checks so the new core behavior stays aligned with current shell-visible behavior while remaining `no_std`
- Establish extraction rules for future pure logic moves so this change becomes the first reusable basis, not a one-off crate split

## Non-Goals / Out of Scope

- Making the whole workspace `no_std` in one change
- Moving provider execution, storage, transport, TUI, hooks, or process management into the core crate
- Rewriting the full turn loop before the first reducer/effect boundary proves out
- Expanding this change into a broad message-type cleanup; adapter translations stay acceptable for this first slice

## First Slice Inventory

- Session inputs moved into the core: `SessionCommand::Prompt`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, loop-state changes used by `StartLoop` / `StopLoop`, and the prompt-completion / post-prompt follow-up inputs currently driven by `notify_prompt_done()` and `check_post_prompt()`
- Shell feedback that must correlate to prior core effects: prompt completion status, loop follow-up completion, and filtered-tool rebuild application
- Core data that must become explicit for the migrated slice: prompt completion input `{effect_id, completion_status}`, post-prompt evaluation input/state `{active_loop_state, pending_follow_up_state, auto_test_enabled, auto_test_command_present, auto_test_in_progress}`, loop-follow-up completion `{effect_id, completion_status}`, and tool-filter application `{effect_id, applied_disabled_tool_set}`
- Primary shell seams for this change: `crates/clankers-controller/src/command.rs`, `crates/clankers-controller/src/auto_test.rs`, `crates/clankers-agent/src/lib.rs`, and `crates/clankers-agent/src/turn/mod.rs`
- `src/modes/` is only impacted if controller/agent adapter signatures change; no direct logic migration is planned there in this change
- Authoritative migrated-slice state owner: `SessionController`; agent-side seams execute controller-owned core effects and return feedback without owning a second reducer

## First Slice Boundary Map

| Shell seam | Entry or feedback path | Intended core boundary | Completion or rejection path |
|---|---|---|---|
| `crates/clankers-controller/src/command.rs` | `SessionCommand::Prompt` | prompt-request input → `StartPrompt` effect | `PromptCompleted` feedback or explicit busy rejection |
| `crates/clankers-controller/src/command.rs` | `SetThinkingLevel` / `CycleThinkingLevel` | thinking-change input → `ApplyThinkingLevel` + logical event effect | immediate shell acknowledgement path |
| `crates/clankers-controller/src/command.rs` | `SetDisabledTools` | tool-filter input → `ApplyToolFilter` + logical event effect | `ToolFilterApplied` feedback on success, or stale-slot rejection with error `DaemonEvent::SystemMessage` and no `DaemonEvent::DisabledToolsChanged` |
| `crates/clankers-controller/src/command.rs` | `StartLoop` / `StopLoop` | loop-control input → loop-state transition + logical event effect | successful `StartLoop` sets the loop active/visible state (`active_loop_id` present / loop shown active) with no immediate acknowledgement event; successful `StopLoop` preserves the visible loop-stop transition plus the current success `DaemonEvent::SystemMessage`; explicit rejections preserve error `DaemonEvent::SystemMessage` |
| `crates/clankers-controller/src/auto_test.rs` | `notify_prompt_done()` / `check_post_prompt()` | prompt-completion / post-prompt evaluation input → follow-up effect or none | successful prompt completion clears busy, emits no dedicated completion acknowledgement, and selects `PostPromptAction::{None, ContinueLoop, RunAutoTest}`; successful `LoopFollowUpCompleted` clears the pending follow-up slot and either advances visible loop iteration/active state for loop continuation or finishes follow-up with no extra acknowledgement event; failed prompt completion clears busy, emits no success acknowledgement, suppresses post-prompt follow-up, and finishes any active loop with error `DaemonEvent::SystemMessage`; failed `LoopFollowUpCompleted` clears the pending follow-up slot, leaves the loop in its failed/inactive visible state, emits error `DaemonEvent::SystemMessage`, and schedules no additional follow-up |
| `crates/clankers-agent/src/lib.rs` and `crates/clankers-agent/src/turn/mod.rs` | thinking/tool-filter application | shell adapter executes controller-owned core effects | agent returns explicit feedback without owning reducer state |

## Parity Baseline

- Prompt/busy parity: preserve busy-flag transitions, one busy rejection path with error `DaemonEvent::SystemMessage`, and no duplicate prompt-start effect
- Thinking parity: preserve effective thinking-level change and the current acknowledgement category (`DaemonEvent::SystemMessage`)
- Disabled-tool parity: preserve disabled-tool state, filtered-tool rebuild completion, `DaemonEvent::DisabledToolsChanged`, and the current acknowledgement category (`DaemonEvent::SystemMessage`); stale-slot rejection preserves prior disabled-tool state, emits only error `DaemonEvent::SystemMessage`, and emits no `DaemonEvent::DisabledToolsChanged`
- Post-prompt parity: preserve busy clearing plus the current next-action categories (`PostPromptAction::{None, ContinueLoop, RunAutoTest}`); successful loop follow-up keeps visible loop iteration/active-state transitions and otherwise finishes with no extra acknowledgement event; failed prompt/loop-follow-up completion suppresses further follow-up and preserves error `DaemonEvent::SystemMessage` for loop-related failure notifications
- Baseline anchors: current controller tests such as `test_reject_concurrent_prompt`, `test_set_thinking_level_valid`, `test_notify_prompt_done_clears_busy`, `test_check_post_prompt_with_auto_test_enabled`, `crates/clankers-controller/src/loop_mode.rs::{test_start_loop,test_stop_loop}`, the `user_tool_filter_*` tests in `crates/clankers-agent/src/turn/mod.rs`, plus explicit `CycleThinkingLevel`, `SetDisabledTools`, and `StartLoop` / `StopLoop` parity regressions

## Verification

- `no_std` boundary rails in ordinary validation: `cargo check -p clankers-core --no-default-features --target thumbv7em-none-eabi`, `scripts/check-clankers-core-boundary.sh`, and the public-core-surface shell-native-type rail (`scripts/check-clankers-core-surface.sh`)
- Reducer coverage for busy gating, prompt start, prompt completion success/failure, `StartLoop` / `StopLoop`, loop continuation, thinking changes, disabled-tool changes, prompt-completion correlation, and out-of-order/mismatched-feedback rejection
- Controller parity coverage in `crates/clankers-controller/src/command.rs`, `crates/clankers-controller/src/auto_test.rs`, and one `SessionController::handle_command(SessionCommand::Prompt { .. })` shell-seam regression, explicitly covering prompt-completion feedback, filtered-tool rebuild application, loop follow-up completion, and explicit `SetDisabledTools`, `CycleThinkingLevel`, and `StartLoop` / `StopLoop` regressions, with ordered assertions where the contract names event order
- Agent parity coverage in `crates/clankers-agent/src/turn/mod.rs` plus one migrated-slice regression around thinking/tool-filter adapter behavior in `crates/clankers-agent/src/lib.rs`
- Continuous verification also includes the dedicated determinism replay rail and the anti-fork review over controller/agent shell seams

## Capabilities

### New Capabilities
- `no-std-functional-core`: A portable `no_std` + `alloc` core crate that owns deterministic orchestration logic through explicit state transitions and effect plans, with `std` crates acting as imperative shells

### Modified Capabilities

## Impact

- `crates/clankers-core/` — new crate for pure state, reducers, effect enums, and invariants
- `crates/clankers-agent/` — consume core plans instead of keeping orchestration policy inline
- `crates/clankers-controller/` — delegate session command state transitions to the core and execute returned shell effects
- `src/modes/` and related runtime wiring — only adapter-signature plumbing if controller/agent integration changes; no direct logic extraction is planned there for this first slice
- Workspace build/test configuration — add `no_std` compile verification and parity/property coverage for extracted logic
