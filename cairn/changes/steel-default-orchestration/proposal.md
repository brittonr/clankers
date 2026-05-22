# Steel Default Orchestration

## Summary

Promote Steel Scheme from an available constrained runtime into the default *planning/orchestration seam* for selected Clankers agent decisions, while preserving Rust as the authority for effects, I/O, verification, receipts, rollback, provider calls, daemon/session state, tool execution, and mutation.

This change does not make Steel ambiently powerful, does not replace Rust enforcement, and does not route arbitrary agent turns through an unrestricted interpreter. It introduces a staged, policy-gated orchestration path where Steel returns typed plans/action envelopes and Rust validates, executes, records, and can fall back to the current Rust-native path.

## Motivation

Clankers now has:

- a constrained Steel Scheme runtime wrapper and CLI,
- typed dynamic-runtime action envelopes,
- Nickel-authored policy/profile rails,
- UCAN-style runtime authority metadata,
- Steel self-mutation preflight/apply/rollback shells,
- Steel/Wasm cross-layer fixtures.

The next step is to make Steel useful as the practical orchestration layer without collapsing the seams that make the architecture reviewable. The first default orchestration slice should be low-risk: Steel proposes turn plans, host actions, routing decisions, or script-selected envelopes; Rust remains the only component that can authorize or apply them.

## Scope

In scope:

- Add a named Steel orchestration profile and policy toggle that can become default for reviewed decision points.
- Add a Rust-owned orchestration adapter seam that loads Steel scripts through the existing runtime wrapper.
- Define typed plan/action outputs that reuse the dynamic-runtime envelope and existing Rust turn/session/tool DTOs where possible.
- Add positive fixtures proving Steel can select/compose plans without host authority.
- Add negative fixtures proving script/profile hot reload cannot expand authority, invent host functions, bypass disabled tools, mutate session state, call providers, or perform I/O.
- Add fallback/kill-switch behavior that returns to Rust-native orchestration when policy disables Steel, script loading fails, or receipt verification fails.
- Add docs and checks that keep CLI, daemon, TUI, attach, controller, provider, and tool-host seams explicit.

Out of scope:

- Replacing the agent loop wholesale in one step.
- Giving Steel direct filesystem, process, git, network, provider, credential, daemon, TUI, or native-tool authority.
- Running untrusted generated code in Steel.
- Real Wasm runtime implementation.
- Making Steel a security sandbox.
- Removing the existing Rust-native orchestration path before parity and rollback evidence exist.

## Non-goals

- Steel does not become the final authority for policy, UCAN verification, receipts, or mutation.
- Steel does not own provider request construction or router fallback/cooldown policy.
- Steel does not own persistent session storage, daemon process state, TUI rendering, or tool execution.
- Nickel remains declarative configuration/policy; UCAN remains runtime delegated authority; Rust remains enforcement.

## Success criteria

- A reviewed policy/profile can enable Steel as default orchestration for at least one narrow decision seam.
- Rust adapters consume Steel output as typed plans/envelopes, not ad hoc strings.
- All host effects still cross existing Rust authorization seams.
- The Rust-native path remains available as fallback and as a comparison oracle during rollout.
- Receipts make it clear whether a decision came from Steel, which profile/script hash was used, which policy was applied, and which host effects were authorized or denied.
