# Steel Agent Turn Wiring

## Summary

Wire the existing `steel.host.plan_turn` orchestration seam into the real Clankers agent-turn path in comparison/default-capable mode, while preserving Rust as the authority for provider requests, tool execution, session/daemon state, receipts, fallback, verification, rollback, and every host effect.

The change makes Steel Scheme practically dogfoodable for one reviewed turn-planning decision. Steel remains a constrained planner that returns typed plans only. Rust parses, checks, authorizes, executes, records, and falls back through existing seams.

## Motivation

Clankers now has the architecture and DTOs for Steel default orchestration:

- Nickel orchestration profile/export rail for `steel.host.plan_turn`,
- `clankers-runtime::steel_orchestration` planner DTOs and receipts,
- dynamic-runtime authorization envelopes for effectful Steel/Wasm requests,
- Steel runtime and self-mutation boundaries,
- docs and checkers proving no ambient Steel authority.

The next high-ROI step is to connect that safe seam to the real agent loop rather than leaving it as a runtime fixture. This should be narrow: one turn-planning hook that can run in comparison mode, emit deterministic receipts, and fall back to the current Rust-native turn behavior without broad agent-loop replacement.

## Scope

In scope:

- Add a Rust-owned agent-turn planning port that real agent/controller code can call before constructing or dispatching model/tool decisions.
- Load/select the `steel.host.plan_turn` profile through Nickel-derived policy data or a checked-in policy fixture path, not script self-selection.
- Invoke `clankers-runtime::steel_orchestration` from the agent-turn shell only through a small adapter/port, never through Steel interpreter internals.
- Preserve Rust-native planning as fallback and comparison oracle.
- Add fixture-backed tests proving Steel planning is called for the selected seam, disabled/failure cases fall back or block according to policy, and any proposed host action still crosses Rust dynamic-runtime authorization before effects.
- Emit bounded deterministic receipts that identify profile/script/policy hashes, plan hash, fallback status, authorization summary, and redaction class.
- Add docs/checker coverage for the real wiring boundary.

Out of scope:

- Replacing the whole agent loop.
- Letting Steel construct provider-native requests directly.
- Letting Steel execute tools, mutate session/daemon/TUI state, access credentials, read/write files, call shell/git/network, or bypass disabled-tool policy.
- Expanding Steel default orchestration to additional seams beyond `steel.host.plan_turn`.
- Real Wasm runtime implementation.
- Treating Steel as a sandbox or untrusted-code boundary.

## Non-goals

- Steel does not become the authority for policy, authorization, provider routing, tool execution, mutation, receipts, or rollback.
- Nickel remains declarative configuration/policy; UCAN remains runtime delegated authority; Rust remains enforcement and host-effect authority.
- The existing Rust-native path remains available until dogfood evidence justifies further rollout.

## Success criteria

- A real agent-turn path calls a Rust-owned Steel planning adapter for `steel.host.plan_turn` when policy enables comparison/default mode.
- The adapter consumes typed `steel_orchestration` DTOs and returns receipts without exposing interpreter internals to agent/controller shells.
- Steel plan items cannot cause host effects unless Rust dynamic-runtime/session/UCAN/disabled-tool checks authorize them.
- Fallback behavior is explicit, receipt-backed, and covered by tests.
- Verification proves enabled, disabled, failure, denied-effect, and repeated deterministic receipt paths.
