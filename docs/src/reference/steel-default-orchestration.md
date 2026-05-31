# Steel Default Orchestration

Steel Scheme is the default planner for the reviewed Clankers `steel.host.plan_turn` turn-planning seam when settings omit `steelTurnPlanning`. Authorized default plans now select the Steel execution adapter, but this does not make Steel an authority boundary. Steel proposes typed plans; Rust-owned host functions remain the authority for I/O, provider calls, tool execution, daemon/session state, mutation, verification, rollback, and receipts.

## Layer split

- Nickel declares the orchestration profile: enabled/default state, exact seam, script identity and hash, runtime budget, fallback mode, allowed host actions, rollout stage, and receipt redaction.
- UCAN-style grants provide runtime delegated authority for the selected planning seam and for the separate `steel.host.execute_turn` execution-authority seam.
- Steel Scheme runs only through the Clankers-owned Steel runtime wrapper and emits typed JSON plan data plus an explicit `(host "steel.host.execute_turn")` JSON host-call request for selected execution.
- Rust parses the JSON plan, builds dynamic-runtime envelopes, authorizes every effect, selects the Steel execution adapter only for authorized default plans, validates the `steel.host.execute_turn` JSON host-call payload, authorizes execution before the host runner, emits receipts, and chooses fallback/block behavior.
- Wasm remains the untrusted/tool execution boundary when a plan selects a Wasm tool.

## Default seam behavior

The first reviewed default planning seam is low-risk turn planning / tool-candidate ordering through `steel.host.plan_turn`. Missing settings use the bundled default profile/script for that seam plus the reviewed `steel.host.execute_turn` host-action entry. Explicit `steelTurnPlanning.enabled = false` remains the kill switch. A profile may run in comparison mode or default mode. Comparison keeps the Rust-native execution oracle; default routes through the Steel-selected execution adapter only after the execute-turn Steel JSON host-call payload is valid and the execution-authority DTO passes dynamic-runtime session capability, UCAN ability, disabled-action, budget, and receipt-destination checks. Extra seams require separate reviewed profile entries, fixtures, and receipts; they do not inherit authority from `steel.host.plan_turn`.

## Fallback and kill switch

If Steel is disabled, malformed, over budget, or fails to evaluate, Rust-native planning is used only when Nickel policy says `fallback_mode = "rust_native"`. If policy says `fallback_mode = "block"`, the planning decision blocks with a stable receipt and no host effect. Fallback must not loosen Steel runtime budgets or silently grant provider, credential, daemon, TUI, filesystem, shell, git, network, or native-tool access.

## Receipt review

Receipts include schema/status, seam, profile, script hash, policy hash, plan hash, Steel runtime receipt hash, execution host-call receipt hash, execution authority receipt hash, authorization receipt summaries, Rust-native fallback status, and redaction decisions. Receipts must not include raw prompts, provider payloads, compact UCAN tokens, raw proofs, credentials, script source, tool bodies, or uncontrolled absolute paths.

## Security wording

Steel default orchestration is a constrained embedded interpreter and planner seam with no ambient authority. It is not an OS/process sandbox. Rust remains the owner of host-function gates, Nickel policy application, UCAN authority, dynamic-runtime authorization, bounded profiles, and deterministic receipts.
