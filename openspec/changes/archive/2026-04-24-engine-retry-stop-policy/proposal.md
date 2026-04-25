## Why

`clankers-engine` now owns the first executable prompt → model → tool → continuation slice. Retry decisions, turn-budget enforcement, and token-limit stop handling still live in `clankers-agent::turn`, which keeps embedders from reusing the full turn policy and leaves the async runtime as the real owner for important terminal outcomes.

## What Changes

- Move retry policy decisions for model failures into engine-owned reducer state and effects, while keeping provider-specific retryability classification and actual sleeping in the host shell.
- Preserve the current default retry envelope in engine-owned policy: two additional attempts, 1-second then 4-second delays, and no jitter.
- Move per-turn model-request budget tracking into the engine so the engine decides when a turn has exhausted its allowed model continuations; the initial request and tool-result follow-up requests count, retries do not.
- Make `StopReason::MaxTokens` an explicit engine-owned terminal path for this slice instead of falling through generic non-tool terminal handling.
- Preserve current shell-visible behavior through adapter tasks and parity rails, but stage the implementation test-first: reducer tests define retry, budget, and token-limit policy before any runtime adapter rewiring.

## Migrated Slice Boundary

In scope for this slice:
- engine inputs for host-classified model failure, retry-ready feedback, cancellation during retry wait, tool feedback that would trigger a follow-up model request, zero-budget prompt submission, and model completion with `StopReason::MaxTokens`; host-classified model failure input carries the pending request identity, failure `message`, optional `status`, and `retryable` flag, while the original structured `AgentError` remains host-side adapter data;
- engine effects and outcomes for retry scheduling, retry exhaustion, non-retryable model failure terminalization, model-continuation budget exhaustion, max-token terminalization, invalid retry feedback rejection, and terminal-failure sidecar reporting;
- `clankers-agent::turn` adapter behavior that executes engine retry/budget effects, preserves original structured `AgentError` values, and feeds classification/cancellation/retry-ready data back to the engine.

Out of scope for this slice:
- provider streaming implementation, provider request shaping, tool execution implementation, hook execution, usage tracking, model-switch polling, daemon protocol, TUI rendering, prompt assembly, session persistence, and automatic max-token continuation.

## Non-Goals

- Do not move prompt assembly, provider I/O, actual backoff waiting, hooks, usage tracking, model switching, daemon protocol, or UI rendering into `clankers-engine`.
- Do not introduce provider-shaped `CompletionRequest`, daemon/TUI types, Tokio handles, timestamps, shell-generated message IDs, or shell-specific request construction into `clankers-engine`.
- Do not change provider-level or router-level retry behavior.
- Do not implement automatic continuation for `StopReason::MaxTokens`; this slice keeps token-limit stops terminal.

## Capabilities

### New Capabilities
- None.

### Modified Capabilities
- `embeddable-agent-engine`: make retry, turn-budget, token-limit, and terminal stop policy concrete engine-owned requirements for the next executable slice.
- `turn-level-retry`: relocate turn-level retry authority from `clankers-agent::turn` loop internals to `clankers-engine` reducer outcomes while preserving retry limits, no-duplicate-message guarantees, and cancellation behavior.

## Implementation Sequence

1. Add failing `clankers-engine` reducer tests for retry scope/reset, retry delay defaults, no message mutation, model-continuation budget counting, budget exhaustion, and max-token terminalization.
2. Extend engine state/input/effect/reducer code until those tests pass.
3. Rewire `clankers-agent::turn` as the imperative shell that executes engine retry/budget effects and reports host-classified failures back to the engine.
4. Add deterministic static and runtime adapter rails before marking verification tasks done.

## Impact

- Affected code: `crates/clankers-engine/src/lib.rs`, `crates/clankers-agent/src/turn/{mod.rs,execution.rs}`, `crates/clankers-agent/src/lib.rs` normal and orchestrated turn-config paths, and FCIS/parity tests in `crates/clankers-controller/tests/fcis_shell_boundaries.rs` or nearby focused rails.
- APIs: expands the engine host contract with retry/budget state and retry-scheduling effects or equivalent engine-native effect data, without adding provider-shaped `CompletionRequest`, daemon/TUI types, Tokio handles, timestamps, or shell-specific request construction to `clankers-engine`.
- Behavior: intended to preserve existing user-visible retry/backoff, max-turn, cancellation, and terminal stop behavior while moving the decision authority into the engine.
- Testing: requires reducer-first positive and negative tests, then adapter parity tests proving the runtime executes engine retry/budget effects instead of re-deriving policy locally.
