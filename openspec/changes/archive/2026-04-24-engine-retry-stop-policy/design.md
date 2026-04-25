## Context

`engine-turn-migration` made `clankers-engine` authoritative for prompt submission, model completion, tool planning, tool feedback, cancellation, and first-slice continuation; it is archived into `openspec/changes/archive/2026-04-24-engine-turn-migration/` and synced into the canonical `embeddable-agent-engine` spec. The next high-ROI gap is the policy still embedded in `crates/clankers-agent/src/turn/mod.rs`: retry loops, retry backoff calculation, per-turn `max_turns` enforcement, and generic handling for `StopReason::MaxTokens` and terminal stop outcomes.

The current runtime shell is async and I/O-heavy. It knows provider retryability through `AgentError::is_retryable()`, sleeps during backoff with Tokio, streams model output, runs tools, emits `AgentEvent`s, and updates usage. Normal turns pass `max_turns: 25` from `Agent::run_turn`, while orchestrated follow-up phases pass `max_turns: 10` from `Agent::execute_orchestrated_turn`. Those values become named adapter constants passed into engine configuration as total model request slots for that turn or orchestration phase, including the initial model request. The policy that decides whether a failed model attempt should retry, how much retry budget remains, whether a follow-up continuation can be minted, and how token-limit stops terminalize belongs in deterministic engine state.

## Goals / Non-Goals

**Goals:**
- Add engine-owned retry and stop-policy state to the host-facing turn contract.
- Make host-supplied retryability classification, engine-owned retry authorization, retry counts, retry delay selection, model-continuation budget, `StopReason::MaxTokens`, and terminal outcome ordering deterministic reducer behavior.
- Stage implementation test-first: add positive and negative `clankers-engine` reducer tests before rewriting the async runtime adapter.
- Preserve existing shell-visible behavior: bounded retry count, exponential backoff shape, cancellation during backoff, no duplicate messages on failed attempts, and terminal error propagation.
- Keep provider/tool I/O and actual sleeping outside the engine.

**Non-Goals:**
- Moving provider streaming, tool execution, hooks, model-switch polling, usage tracking, or prompt assembly into `clankers-engine`.
- Replacing provider-level or router-level retry policy.
- Introducing automatic continuation for `StopReason::MaxTokens`; this slice terminalizes token-limit stops explicitly and leaves auto-continue for a later spec.
- Reworking daemon, TUI, attach, or session persistence APIs beyond adapter/parity seams needed to prove the migrated policy.
- Adding new `clanker-message::StopReason` variants.

## Decisions

### 1. Engine decides retry policy, host supplies retryability and executes waits

**Choice:** `EngineInput::ModelFailed` will carry engine-native host-classified failure data: pending model request identity, failure `message`, optional provider/status code `status`, and provider-classified `retryable` flag. The original structured `AgentError` stays in the `clankers-agent::turn` adapter as host-side data correlated with the pending request identity; the engine sidecar records only `EngineTerminalFailure { message, status, retryable }`. The engine owns retry authorization under the current state, retry attempt counts, and delay selection. Retry budget is scoped to one pending model request: retry effects preserve that same pending model request identity, success clears that request's counter, and later follow-up model requests receive a fresh retry budget. The host owns provider/error classification before feedback submission and sleeps when the engine returns retry/backoff work.

**Rationale:** The engine must stay deterministic and provider-agnostic. It should not parse HTTP errors or depend on provider-specific types, but embedders need one reusable policy for how classified failures advance the turn.

**Alternative considered:** Keep retry policy fully in `clankers-agent::turn` because that code already sees `AgentError`.

**Why not:** That leaves embedders unable to reuse retry semantics and keeps a second authoritative stop-policy copy above the engine boundary.

### 2. Retry scheduling is an effect, not async engine behavior

**Choice:** the engine will return a distinct `EngineEffect::ScheduleRetry { request_id, delay: core::time::Duration }` effect and move into a retry-waiting phase. The effect carries the same pending model request identity. The default policy preserves the current runtime constants: at most two additional attempts, 1 second before the first retry, 4 seconds before the second retry, and no jitter. After the host waits, it reports an engine-native retry-ready input with the same request ID; the engine then re-emits `EngineEffect::RequestModel` for the same pending model request identity.

If cancellation fires while the host is waiting on a scheduled retry delay, the adapter must submit `EngineInput::CancelTurn { reason: "turn cancelled" }` for the still-pending retry-waiting engine state before returning `AgentError::Cancelled`. The engine clears the pending model request and emits cancellation terminal output in this exact order: `BusyChanged(false)`, `Notice("turn cancelled")`, then `TurnFinished(Stop)`. The adapter must not report retry-ready or execute another model attempt after cancellation. Any later retry-ready, model-success, or model-failure feedback for the cancelled request is rejected without state mutation or effects.

**Rationale:** Reducers remain pure, deterministic, and testable without Tokio. The host remains the imperative shell for time and I/O.

**Alternative considered:** Put async timers or Tokio primitives inside the engine.

**Why not:** That would violate the functional-core / imperative-shell boundary and make engine tests slower and less deterministic.

### 3. Turn budget moves into engine state with per-submission configuration

**Choice:** the migrated engine submission/template carries the per-turn or per-phase model-continuation budget currently represented by `TurnConfig::max_turns`. `EngineState` tracks model request slots for the turn/phase: the budget must be at least one, the initial model request consumes one slot, each tool-result follow-up model request consumes one additional slot, and retries of the same pending model request do not consume slots because retry budget is separate. The default adapters preserve the current normal-turn budget of 25 total slots and orchestration follow-up phase budget of 10 total slots through named constants passed into engine configuration; both totals include the initial model request for that turn/phase. A zero budget is rejected at prompt submission with an engine-native invalid-budget rejection, no effects, and unchanged state; the adapter surfaces it through the existing engine-rejection error path without starting provider I/O. If accepted tool feedback would exceed the continuation budget, the engine records that accepted tool feedback in canonical messages first, then clears pending tool work and emits `BusyChanged(false)`, `Notice("engine model request slot budget exhausted")`, then `TurnFinished(Stop)`.

**Rationale:** The decision to continue after tool results is already engine-owned. Enforcing the continuation budget in the shell would keep split-brain continuation policy.

**Alternative considered:** Leave the `for turn_index in 0..config.max_turns` loop as the budget owner and migrate only retry.

**Why not:** Tool-feedback continuation decisions and turn-budget terminalization are inseparable: a host-facing engine must tell embedders when another model request is allowed.

### 4. `StopReason::MaxTokens` terminalizes explicitly in this slice

**Choice:** the engine will branch on `StopReason::MaxTokens` as a named terminal path with deterministic events/outcome. It first accepts the model completion by appending assistant content to canonical engine messages, then clears pending model work and emits terminal events in the same order as other successful terminal stops: `BusyChanged { busy: false }` before `TurnFinished { stop_reason: StopReason::MaxTokens }`. It will not auto-continue or retry token-limit completions in this change.

**Rationale:** The current behavior treats max-token stops as terminal. Making that explicit closes the spec/test gap without changing user-visible semantics.

**Alternative considered:** Automatically request a continuation after max-token stops.

**Why not:** Auto-continuation changes behavior, affects token budgets and prompts, and needs a separate product decision.

### 5. Terminal paths share one engine-owned terminalization helper

**Choice:** normal stop, max-token stop, non-retryable model failure, retry exhaustion, budget exhaustion, and cancellation should flow through engine-owned terminalization helpers that clear pending work and emit consistent semantic events. Model-failure terminalization emits `BusyChanged(false)`, a `Notice` with the failure message, then `TurnFinished(Stop)`, and sets `EngineOutcome.terminal_failure = Some(EngineTerminalFailure { message, status, retryable })` from the latest host-supplied failure details. The adapter must keep the original structured `AgentError` as a sidecar while asking the engine to authorize retry or terminalization; after terminalization, it returns that original `AgentError` and uses `terminal_failure` only as the engine-owned audit/authorization result, not as a lossy reconstruction source. Budget exhaustion emits `BusyChanged(false)`, a budget-exhaustion `Notice`, then `TurnFinished(Stop)` with no terminal failure. Max-token terminalization emits `BusyChanged(false)` then `TurnFinished(MaxTokens)` with no terminal failure. Invalid zero-budget submission returns `EngineRejection::InvalidBudget`, no effects, unchanged state, and no terminal failure.

**Rationale:** Centralizing terminal state reduces drift and makes adapter parity easier to test. The host can still translate semantic events into existing `AgentEvent`s or returned errors.

**Alternative considered:** Add separate shell-side cleanup for each terminal path.

**Why not:** Separate cleanup paths are exactly the duplicated terminal policy this slice is meant to remove.

### 6. Implementation starts with reducer tests, then adapter migration

**Choice:** the first implementation task adds failing `clankers-engine` tests for retry scheduling, retry identity preservation, matching retry-ready re-emitting the model request from the retry-waiting phase, exact default delays (1 second, then 4 seconds, no jitter, and no delay after retry exhaustion), retry exhaustion, non-retryable failure, no message mutation, cancellation while retry is scheduled, turn budget, max-token terminalization, and negative rejection paths. Mismatched request IDs, including mismatched retry-ready, return `CorrelationMismatch` with unchanged state and no effects. Wrong phase, duplicate model-failure feedback while retry is already scheduled, model-success/model-failure feedback during retry-wait before matching retry-ready, and retry-ready/model-success/model-failure feedback after retry-wait cancellation or terminalization return `InvalidPhase` with unchanged state and no effects. Only after those tests define the target should the engine API and runtime adapter change.

**Rationale:** Tests first keep the new boundary reviewable and prevent async runtime details from defining the engine contract by accident.

**Alternative considered:** Rewrite `run_turn_loop` first and backfill tests.

**Why not:** That risks preserving runtime-local policy behind a thin engine wrapper instead of moving authority into reducers.

## Risks / Trade-offs

- **[API churn]** → Engine state/effect/input types are still young. Keep new fields narrow and tied to retry/budget/stop policy only.
- **[Behavior drift]** → Retry and cancellation paths are user-visible under failure. Preserve existing focused runtime tests and add adapter parity tests before deleting shell-owned constants.
- **[Retry identity coupling]** → Retrying preserves the same pending model request identity for this slice. If a future provider requires fresh external request IDs per attempt, keep that as host-local execution metadata rather than changing engine correlation semantics.
- **[Host classification boundary]** → The engine cannot know provider-specific retryability. Carry a host-classified `retryable` flag and failure details in engine-native input rather than importing provider error types.
- **[Backoff unit leakage]** → Avoid `tokio::time::Duration` in public reducer types. Use `core::time::Duration` in `EngineEffect::ScheduleRetry` so tests stay deterministic and the runtime shell can convert to Tokio sleep at the adapter edge.
- **[Engine surface leakage]** → Do not introduce provider-shaped `CompletionRequest`, daemon/TUI types, Tokio handles, timestamps, shell-generated message IDs, or shell-specific request construction into retry/budget/stop-policy effects; keep conversion to provider requests in `crates/clankers-agent/src/turn/execution.rs`.
- **[Over-scoping]** → Do not combine this with model switching, usage accounting, prompt assembly, or session persistence changes.

## Migration Plan

1. Add reducer-first tests in `crates/clankers-engine` for retry identity preservation, retry scheduling, matching retry-ready request re-emission, exact 1-second and 4-second retry delays with no jitter and no delay after exhaustion, retry exhaustion, non-retryable failure details, cancellation while retry is scheduled, model-success/model-failure feedback during retry-wait before matching retry-ready, mismatched request IDs including mismatched retry-ready returning `CorrelationMismatch`, feedback after retry-wait cancellation, wrong phase / duplicate failure / retry after terminalization returning `InvalidPhase`, budget behavior, max-token behavior, and terminal stop policy.
2. Extend engine input/state/effect/outcome types to satisfy those tests while keeping provider I/O and sleeping outside the engine.
3. Move retry and turn-budget decisions out of `clankers-agent::turn` so the runtime executes engine effects and reports classified failure/cancellation feedback.
4. Add a deterministic static FCIS-style source inventory plus focused runtime adapter tests that reject reintroduced local retry counts, retry delays, turn-budget enforcement, and token-limit decisions outside `clankers-engine`. The static rail inventories non-test `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}` for forbidden retry-budget/backoff constants, retry-delay arithmetic, direct `config.max_turns` continuation loops/branches, and `StopReason::MaxTokens` terminal-policy branches. It explicitly allows matching `EngineEffect::ScheduleRetry`, sleeping for the engine-provided delay, executing engine `RequestModel` effects, parsing provider stop strings into `StopReason`, provider request conversion in `turn/execution.rs`, and named normal/orchestration budget constants in `crates/clankers-agent/src/lib.rs` that only feed engine configuration. Runtime adapter tests must prove: retryable provider failure recovers through engine `ScheduleRetry`; retry exhaustion and non-retryable failure propagate the original structured `AgentError` after engine terminalization; cancellation during engine-scheduled retry wait returns `AgentError::Cancelled` without retry-ready feedback or another provider/model attempt; failed retry attempts do not duplicate shell-visible messages; normal `Agent::run_turn` passes 25 total model request slots and orchestrated follow-up phases pass 10 total model request slots into engine configuration; model-continuation budget exhaustion emits no follow-up model request; zero-budget rejection uses the existing engine-rejection error path and starts no provider I/O; and `StopReason::MaxTokens` terminalizes without retry/tool/follow-up effects while remaining shell-visible.
5. Run focused engine, agent, and boundary tests; capture machine-produced evidence under the change before marking verification tasks done.

## Open Questions

- None.
