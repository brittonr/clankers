# Steel Turn Planning UCAN Authority

## Summary

Bind Steel Scheme turn-planning activation to explicit UCAN-backed runtime authority before `steel.host.plan_turn` can influence a real agent turn. Nickel continues to declare the reviewed profile/script/budget contract, Steel continues to return typed plans only, and Rust remains the enforcement, provider, fallback, receipt, and execution authority.

## Motivation

Clankers now has a reviewed settings path and deterministic runtime smoke proving config-selected Steel turn planning reaches a real session prompt. The remaining seam risk is that activation currently relies on static settings/session capability shape rather than a first-class UCAN invocation decision for the Steel planning authority itself.

This change makes the runtime authority boundary explicit and testable:

```text
Nickel = declared planning policy/profile/script/budget
UCAN   = runtime delegated authority for steel.host.plan_turn
Rust   = validates UCAN, checks policy/session, emits receipts, executes/fallbacks
Steel  = trusted typed planning logic with no ambient authority
Wasm   = untrusted/tool execution boundary, not widened by this change
```

## Scope

In scope:

- Define the UCAN ability/resource vocabulary for Steel turn planning.
- Add a Rust-owned authority adapter seam that evaluates reviewed settings, session context, and UCAN invocation metadata before Steel planning runs.
- Preserve disabled-by-default behavior and existing fail-closed profile/script/hash checks.
- Emit deterministic redacted authority receipts for allowed and denied planning decisions.
- Add focused tests for allowed grants and missing/expired/revoked/wrong-scope/overbroad grants.
- Add a checker receipt under `target/steel-turn-planning-ucan-authority/`.

Out of scope:

- Giving Steel filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session-mutation, code-mutation, or tool-execution authority.
- Replacing Rust provider/tool/session execution authority.
- Implementing general UCAN effect-permissions for every Clankers effect class.
- Treating Steel as a sandbox or as untrusted generated code.

## Expected Outcome

A reviewed Steel turn-planning config may only run when Rust verifies a matching UCAN grant for the normalized `steel.host.plan_turn` planning resource. Invalid or absent authority fails closed before Steel execution and before provider/tool calls are needed to hide the denial. Receipts record safe proof metadata without raw compact tokens, signing material, prompts, profile bodies, or script bodies.
