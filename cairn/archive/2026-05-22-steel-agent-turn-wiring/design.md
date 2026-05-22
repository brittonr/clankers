# Design: Steel Agent Turn Wiring

## Architecture split

The wiring keeps the existing architecture rule intact:

```text
Nickel = declared intent/config/policy/contracts
UCAN   = runtime delegated authority
Rust   = enforcement, I/O, receipts, verification, rollback
Steel  = trusted orchestration/request/planning logic
Wasm   = untrusted/tool execution boundary
```

Steel is wired into the real turn path as a planner only. It returns typed plans. Rust owns every effectful consequence.

## Real turn-planning port

Add or reuse a Rust-owned port at the agent-turn boundary, conceptually:

```rust
trait AgentTurnPlanner {
    fn plan_turn(&self, input: TurnPlanningInput) -> OrchestrationPlanReceipt;
}
```

The real agent/controller shell calls this port at the selected planning point for `steel.host.plan_turn`. The shell must not import Steel interpreter APIs, read Steel source directly, or branch on interpreter details. It may choose between a Rust-native planner and a Steel-backed planner based on policy/profile state.

## Adapter boundary

The Steel-backed implementation delegates to `clankers-runtime::steel_orchestration` and the existing Steel runtime wrapper. It converts the real agent-turn context into `TurnPlanningInput` using safe summaries/hashes rather than raw secrets or unbounded provider/session material.

The adapter returns:

- accepted typed plan receipts,
- fallback receipts when policy allows Rust-native fallback,
- blocked receipts when fallback is disabled,
- denial receipts for unauthorized requested host actions.

## Policy selection

Nickel policy/profile data decides whether `steel.host.plan_turn` is disabled, comparison-only, or default for the selected seam. Scripts cannot self-select default status, add host functions, expand budgets, or override fallback mode.

Runtime authority for requested actions still uses UCAN/session/disabled-tool checks and dynamic-runtime authorization. Policy selection only chooses the planner path; it does not grant effects.

## Effect authorization

A Steel plan item that would cause a host effect must be lowered to the existing Rust action/envelope authorization path before execution. The wiring must explicitly preserve these owners:

- provider/router adapters own provider request construction, retry, fallback, and cooldown semantics,
- tool hosts own tool execution and disabled-tool checks,
- daemon/controller/session shells own session state mutation,
- mutation requests go through Steel self-mutation preflight/apply/rollback,
- dynamic-runtime authorization records no-write denial/approval receipts before any effect.

## Fallback and comparison mode

The first wiring should support comparison mode: Steel planning runs and produces a receipt while the Rust-native planner remains the execution oracle unless policy explicitly selects Steel as default for the seam.

Fallback must be deterministic and explicit:

- disabled profile: Rust-native only, no Steel-authored decision claim,
- script load/eval/parse failure: fallback receipt if allowed,
- unsupported/denied host action: no host effect; fallback only if the failed Steel plan has not already requested an unsafe effect and policy allows fallback,
- fallback disabled: block with stable issue code.

Fallback must never retry Steel under a looser profile.

## Receipt model

Receipts should be bounded and secret-free. They should include stable identifiers for:

- planning seam (`steel.host.plan_turn`),
- policy/profile/script hashes,
- plan schema and plan hash,
- selected mode (`disabled`, `comparison`, `default`),
- fallback status and reason,
- authorized/denied action summaries,
- redaction class,
- repeated-run stability hash where practical.

Raw prompts, credentials, provider payloads, tool outputs, session transcripts, and raw Steel source bodies must not be emitted in receipts.

## Verification strategy

Implement fixture-backed tests around the real adapter/agent-turn boundary:

1. enabled comparison mode calls Steel and records a comparison receipt while preserving Rust-native execution;
2. default mode consumes a typed Steel plan only after Rust authorization;
3. disabled profile bypasses Steel and records Rust-native planning without Steel claims;
4. malformed/script-failure Steel output falls back or blocks according to policy;
5. Steel-requested provider/tool/session/mutation action is denied unless it crosses the existing Rust authorization seam;
6. repeated identical fixture inputs produce stable receipt hashes;
7. architecture checker prevents agent/controller/daemon/TUI/provider shells from importing interpreter internals directly.

## Rollout

This change only wires `steel.host.plan_turn`. Additional seams require separate reviewed profile entries, fixtures, receipts, and Cairn changes.
