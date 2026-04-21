## Context

`SessionController` is currently described as the transport-agnostic core, but its state transitions still live inside `std` and Tokio-oriented code that also owns event emission, persistence, hook execution, tool rebuilding, and prompt orchestration. `Agent` has the same pattern: deterministic policy and runtime side effects are interleaved in one place. That makes the logic hard to test as a pure unit and easy to regress when shell concerns change.

A `no_std` boundary is useful here because it is a hard architectural constraint, not a style preference. If the extracted core compiles with `alloc` only, then filesystem, network, clock, process, terminal, tracing, and async runtime dependencies cannot leak back in accidentally. That makes `no_std` the forcing function for the first real functional-core / imperative-shell split.

This change is intentionally a first slice, not a workspace-wide conversion. The main value is to establish one end-to-end pattern that future extractions can copy.

## Goals / Non-Goals

**Goals:**
- Introduce a new `clankers-core` crate that builds as `#![no_std]` with `alloc`
- Extract one meaningful orchestration slice into pure state transitions instead of moving only data structs
- Make shell work explicit through effect plans returned by the core
- Keep `clankers-agent`, `clankers-controller`, and runtime modes as imperative adapters that translate I/O into core inputs and execute returned effects
- Add deterministic verification rails: pure tests, negative-transition tests, parity tests, and a `no_std` compile check

**Non-Goals:**
- Make the whole workspace `no_std`
- Move TUI, provider, redb, hooks, Matrix, QUIC, or process management into the core
- Rewrite the full agent turn loop in one change
- Freeze every future extraction decision now; this change only establishes the first pattern

## Decisions

### 1. Create a new `clankers-core` crate instead of feature-gating existing runtime crates

**Choice:** Add `crates/clankers-core` as a new workspace crate with a `#![no_std]` baseline. If the crate later exposes a `std` feature for tests or adapter-facing conveniences, acceptance still requires a bare-metal `no_std` rail: `cargo check -p clankers-core --no-default-features --target thumbv7em-none-eabi`.

**Rationale:** `clankers-agent` and `clankers-controller` already depend on Tokio, broadcast channels, persistence, hooks, and runtime-oriented crates. Feature-gating those crates down to `no_std` would turn this change into dependency surgery before any functional core appears. A fresh crate creates a clean boundary immediately.

**Alternatives considered:**
- **Feature-gate `clankers-controller` or `clankers-agent` directly:** Rejected because existing `std` dependencies are too entangled for a first slice.
- **Keep pure modules inside existing crates without `no_std`:** Rejected because it does not enforce the boundary and will drift back toward imperative code.

### 2. Extract a reducer-style core API, not async traits

**Choice:** Represent the first slice as pure transition functions over explicit inputs and state, returning `(next_state, effects, outcome)`-style data. `effects` is an ordered plan, and shell adapters must preserve that order whenever shell-visible sequencing matters.

**Rationale:** Async traits or callback interfaces would hide side effects behind dynamic calls and let ambient runtime state leak into the core. Reducers force every decision to depend on explicit inputs and make deterministic tests trivial.

**Implementation shape:**
- `CoreState` owns migrated session orchestration state
- `CoreInput` represents user commands and shell feedback
- `CoreEffect` describes shell work to perform
- `CoreOutcome` / `CoreError` describes accepted, rejected, or terminal transitions
- Rejections use a typed `RejectedTransition`-style outcome that guarantees state is unchanged
- Shell feedback that completes prior work carries a correlation token from the originating `CoreEffect`; if the current state has no matching pending work, the core returns a typed rejection and leaves state unchanged

**Alternatives considered:**
- **Async trait ports in the core:** Rejected because they preserve imperative structure and make side effects less visible, not more.
- **Closure/callback-based orchestration:** Rejected because it recreates hidden control flow and implicit ambient state.

### 3. Scope the first slice to session command and prompt lifecycle decisions

**Choice:** The initial migrated slice will cover `SessionCommand::Prompt`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, loop-state changes used by `StartLoop` / `StopLoop`, and the prompt-completion / post-prompt follow-up paths currently driven by `notify_prompt_done()` and `check_post_prompt()`.

**First-slice mapping:**
- `crates/clankers-controller/src/command.rs` continues to accept those session commands, but the decision logic for the named slice moves into `clankers-core`
- `crates/clankers-controller/src/auto_test.rs` continues to host prompt-completion/post-prompt adapter code, but `notify_prompt_done()` and `check_post_prompt()` decisions are driven by `clankers-core`
- `crates/clankers-agent/src/lib.rs` and `crates/clankers-agent/src/turn/mod.rs` remain shell seams for thinking/tool-filter application and turn execution, but migrated policy comes from `clankers-core`
- Loop follow-up completion, prompt completion feedback, and filtered-tool rebuild application are explicit correlated feedback paths in scope for this first slice

**Core state ownership:** The authoritative `CoreState` for the migrated slice lives in `SessionController`. `clankers-agent` does not run an independent reducer or hold a second authoritative copy; it applies controller-requested shell effects, uses the same `clankers-core` effect/input/state contracts for the migrated slice, and returns explicit feedback inputs to the controller-owned core state machine.

**Loop-control rejection rules:** `StartLoop` is accepted only when no loop is already active for the session core state; otherwise the core returns a typed `LoopAlreadyActive` rejection. `StopLoop` is accepted only when a loop is active; otherwise the core returns a typed `LoopNotActive` rejection. If loop follow-up work is already pending, conflicting loop-control inputs are rejected until the pending follow-up resolves or is explicitly cleared by the core.

**Disabled-tool rejection rule:** `SetDisabledTools` is accepted only when no prior `ApplyToolFilter` completion is still pending. If a tool-filter rebuild slot is already pending, the core returns a typed stale-slot rejection and leaves the previously accepted disabled-tool state unchanged.

**Rationale:** This slice is central enough to matter, already spans `SessionController` and `Agent`, and can be modeled as explicit transitions plus effects. It gives a real functional core without needing to pull provider execution, storage, or transport code into `no_std` immediately.

**Alternatives considered:**
- **Move only message/data structs first:** Rejected because it would produce a portable crate without a meaningful functional core.
- **Move the full turn loop first:** Rejected because provider/tool streaming, hooks, and persistence make that too large for the first boundary-setting change.

### 4. Keep core effects domain-level, not shell/protocol-specific

**Choice:** `clankers-core` emits domain-level effects such as "start prompt", "emit logical session event", "rebuild filtered tools", or "continue loop" rather than raw `DaemonEvent`, `AgentEvent`, Tokio tasks, or transport/protocol frames.

**First-slice effect / feedback matrix:**

| `CoreEffect` intent | Feedback expected? | Correlation identity | Matching `CoreInput` | Pending state resolved |
|---|---|---|---|---|
| `StartPrompt` | Yes | core-owned prompt effect ID | `PromptCompleted { completion_status }` | busy/prompt lifecycle |
| `ApplyToolFilter` | Yes | core-owned tool-filter effect ID | `ToolFilterApplied { applied_disabled_tool_set }` | disabled-tool rebuild application |
| `RunLoopFollowUp` | Yes | core-owned loop-follow-up effect ID | `LoopFollowUpCompleted { completion_status }` | loop continuation / post-prompt follow-up |
| `ApplyThinkingLevel` | No | n/a | n/a | thinking configuration |
| `EmitLogicalSessionEvent` | No | n/a | n/a | shell-visible event fan-out |

This table closes the first slice for this change. Additional feedback-bearing effects are out of scope until a follow-up change extends the table and its verification.

**Pending-work model:** `CoreState` keeps one shared monotonic pending-work counter plus three optional pending slots: prompt, tool-filter rebuild, and loop follow-up. Every feedback-bearing effect draws its identity from that shared core-owned counter, so identities are unique across the entire first-slice namespace even when different pending kinds coexist. Completion inputs must match both slot kind and identity; empty-slot, wrong-kind, duplicate, or stale completions are rejected without state mutation.

**Completion-failure semantics:** Failed `PromptCompleted` always clears busy state, emits no success acknowledgement, and suppresses post-prompt follow-up. If a loop was active for that prompt, the core transitions that loop into the failed/inactive visible state and preserves error `DaemonEvent::SystemMessage` for the loop-failure notification. Failed `LoopFollowUpCompleted` clears the pending follow-up slot, leaves the loop in its failed/inactive visible state when the follow-up was loop-owned, preserves error `DaemonEvent::SystemMessage` for the failed follow-up notification, and never schedules another follow-up from that failed completion.

**Rationale:** The core should stay reusable across standalone, daemon, attach, and future runtimes. If it emits shell-native protocol types directly, the boundary becomes coupled to one shell instead of defining portable intent.

**Alternatives considered:**
- **Return raw `DaemonEvent` / `AgentEvent` values from the core:** Rejected because those are shell/protocol concerns and pull too much runtime knowledge into the core.
- **Make the effect enum totally generic:** Rejected because a vague command bus would make adapters harder to reason about. Keep the enum concrete for the migrated slice.

### 5. Use alloc-friendly data and temporary adapter translations

**Choice:** The core uses `alloc` types and plain value objects. Time, IDs, diagnostics, and runtime results are supplied by the shell as explicit data rather than being fetched inside the core. For this first slice, existing `clankers-message` and related shell-native data types stay outside the core unless one specific migrated type proves impossible to express without them.

**Rationale:** `chrono`, `tokio`, paths, environment access, and filesystem handles are all shell concerns. Keeping core payloads simple avoids pulling in `std`-only transitive dependencies and keeps serialization/test support straightforward. Temporary adapter translations are a smaller risk than ballooning the first slice into a cross-crate type migration.

**Alternatives considered:**
- **Mirror existing runtime types wholesale:** Rejected because many current types already encode shell/runtime assumptions.
- **Introduce broad compatibility shims for `std` types:** Rejected because the shims obscure the boundary instead of clarifying it.
- **Move `clankers-message` wholesale in the first change:** Rejected because it widens scope before the reducer/effect pattern is proven.

### 6. Verify the boundary with both compile and behavioral rails

**Choice:** Verification includes a dedicated bare-metal `no_std` rail for `clankers-core`, pure positive/negative tests for the reducer, and parity tests through named `std` shell seams.

**Determinism rail:** Add a dedicated `clankers-core` reducer test that replays identical migrated-slice state/input pairs twice and asserts identical next state plus identical effect plans. Wire this rail into the repo's ordinary validation path alongside the boundary scripts and the bare-metal `cargo check -p clankers-core --no-default-features --target thumbv7em-none-eabi` rail so it runs continuously, not only on demand.

**Reducer rail:** Pure-core reducer tests explicitly cover the transitions corresponding to prompt start/completion, `notify_prompt_done()`, `check_post_prompt()`, `StartLoop` / `StopLoop`, thinking changes, disabled-tool changes, and the required rejection paths.

**Dependency/API boundary rail:** Add a persistent repo check at `scripts/check-clankers-core-boundary.sh` that fails if `clankers-core` gains banned direct dependencies or imports. The authoritative denylist for this change is: `tokio`, `tokio-util`, `crossterm`, `ratatui`, `redb`, `reqwest`, `iroh`, `portable-pty`, `std::fs`, `std::net`, `std::process`, `std::env`, `std::time`, `chrono::`, `tokio::`, `crossterm::`, `ratatui::`, `redb::`, `reqwest::`, and `iroh::`. The script runs against `crates/clankers-core/Cargo.toml` plus `crates/clankers-core/src/**/*.rs` and fails the change if any denied dependency or import appears. Wire that script into the repo's ordinary validation path so the rail runs continuously, not only by manual invocation.

**Public core surface rail:** Add `scripts/check-clankers-core-surface.sh` as a persistent repo check over exported `clankers-core` boundary types so `CoreState`, `CoreInput`, `CoreEffect`, `CoreOutcome`, `CoreError`, and other public core API types cannot leak shell-native or protocol-native types back across the adapter boundary. Wire this rail into the repo's ordinary validation path alongside the other boundary rails.

**Parity seams:**
- Extend `crates/clankers-controller/src/command.rs` coverage for `SetThinkingLevel`, `CycleThinkingLevel`, concurrent-prompt rejection, disabled-tool updates, filtered-tool rebuild application, logical session-event emission, and `StartLoop` / `StopLoop` through the migrated reducer path
- Extend `crates/clankers-controller/src/auto_test.rs` coverage for prompt completion, loop continuation, `notify_prompt_done()`, `check_post_prompt()`, correlated loop follow-up completion, and preserved `PostPromptAction::{None, ContinueLoop, RunAutoTest}` categories through the migrated reducer path
- Add one controller shell-seam regression that drives `SessionController::handle_command(SessionCommand::Prompt { .. })` through the migrated reducer/effect path and asserts preserved prompt shell-visible categories, emitted events, plus busy-state transitions
- Extend `crates/clankers-agent/src/turn/mod.rs` coverage for migrated tool-filter behavior through the core adapter path
- Add one agent-side regression in `crates/clankers-agent/src/lib.rs` that exercises migrated thinking/tool-filter adapter behavior through `clankers-core`
- Add matching and mismatched correlation-token round-trip coverage for prompt completion, loop follow-up completion, and filtered-tool rebuild application, including failed completion outcomes
- Add explicit empty-slot, wrong-kind, duplicate, stale, and stale-slot rejection coverage for those same feedback paths

**Parity baseline:**
- Prompt requests while busy still reject without duplicate prompt execution
- Prompt completion feedback still clears busy state, emits no dedicated completion acknowledgement event on success, and drives only `PostPromptAction::{None, ContinueLoop, RunAutoTest}` follow-up choices; failed completion suppresses follow-up, emits no success acknowledgement, and preserves loop-related failure notification through the error `DaemonEvent::SystemMessage` category when an active loop is failed
- Successful loop-follow-up completion clears the pending follow-up slot and advances visible loop iteration/active-state transitions for loop continuation; non-loop follow-up completion finishes with no extra acknowledgement event
- Failed loop-follow-up completion preserves error `DaemonEvent::SystemMessage`, leaves the loop in its failed/inactive visible state, and schedules no additional follow-up
- `StartLoop` / `StopLoop` and loop-follow-up completion still preserve loop-visible state transitions; successful `StartLoop` sets active/visible loop state and emits no immediate acknowledgement event, successful `StopLoop` keeps the success `DaemonEvent::SystemMessage` category, and rejection paths keep the error `DaemonEvent::SystemMessage` category
- Thinking-level changes still preserve the exact cycle order `Off → Low → Medium → High → Max → Off`; invalid `SetThinkingLevel` inputs still leave state unchanged and emit the error `DaemonEvent::SystemMessage` category
- Disabled-tool changes still rebuild the filtered tool set and emit `DaemonEvent::DisabledToolsChanged` before the `DaemonEvent::SystemMessage` acknowledgement after successful rebuild application; stale-slot rejection leaves disabled-tool state unchanged, emits only the error `DaemonEvent::SystemMessage`, and emits no `DaemonEvent::DisabledToolsChanged`

**Anti-fork review boundary:**
- In `crates/clankers-controller/src/command.rs`, the migrated decision branches for `SessionCommand::Prompt`, `SetThinkingLevel`, `CycleThinkingLevel`, `SetDisabledTools`, `StartLoop`, and `StopLoop` must reduce to adapter translation plus effect execution
- In `crates/clankers-controller/src/auto_test.rs`, `notify_prompt_done()` and `check_post_prompt()` must reduce to adapter translation plus effect execution
- In `crates/clankers-agent/src/lib.rs`, thinking/tool-filter handling remains shell application logic only and must not keep independent migrated-slice policy branches
- In `crates/clankers-agent/src/turn/mod.rs`, any migrated tool-filter behavior remains execution-time shell wiring only and must not duplicate controller-owned reducer policy

**Rationale:** A successful `no_std` compile proves the boundary exists, but not that behavior stayed correct. Pure reducer tests and shell parity tests together enforce both architectural and behavioral intent.

## Risks / Trade-offs

- **Boundary too large for a first extraction** → Fix scope to the session-command/prompt-lifecycle slice and defer other logic to follow-up changes.
- **Type duplication and translation drift between core and shell** → Keep the migrated slice narrow, add focused adapter tests, and prefer domain-level core types over wholesale copies of shell structs.
- **Effect enum grows into an unreviewable kitchen sink** → Limit effects to the chosen slice and require new variants to correspond to explicit migrated behavior.
- **`no_std` dependency friction from existing crates** → Keep `clankers-core` dependency-light and pass shell metadata in plain values instead of reusing `std`-heavy types.
- **Behavior regressions during cutover** → Preserve existing controller/agent tests for the migrated slice and add parity assertions before deleting old paths.

## Migration Plan

1. Create `crates/clankers-core` with `#![no_std]`, `alloc`, core-friendly dependencies, and a compile rail that proves the crate builds without `std`.
2. Define the first-slice state, input, effect, and outcome types in the new crate.
3. Port the targeted session-command/prompt-lifecycle logic into pure transition functions with positive and negative tests.
4. Add shell adapters in `clankers-controller` / `clankers-agent` that translate runtime events into `CoreInput` and execute `CoreEffect` outputs. `src/modes/` only takes any necessary adapter-signature plumbing; it is not a direct logic-migration target in this change.
5. Run parity tests through the named controller and agent `std` integration points, then remove the duplicated in-shell orchestration logic for the migrated slice.
6. Leave follow-up extractions for later changes once the reducer/effect pattern proves stable.

## Open Questions

No blocking open questions. This change intentionally keeps `clankers-message` behind adapter translations unless a specific migrated type cannot be expressed otherwise, and parity work is anchored to `crates/clankers-controller/src/command.rs`, `crates/clankers-controller/src/auto_test.rs`, one required controller shell-seam regression, `crates/clankers-agent/src/turn/mod.rs`, and one required agent-side regression in `crates/clankers-agent/src/lib.rs`.
