# Design: Steel Turn Planning UCAN Authority

## Overview

This change introduces a narrow Rust-owned authority gate for the already-wired Steel Scheme turn-planning seam. The gate sits after settings/profile/script validation and before any Steel planner invocation. Its input is normalized activation data, session context, and host-owned UCAN proof context; its output is an allowed/denied decision plus safe receipt metadata.

The gate does not execute Steel, provider calls, tools, daemon mutations, or filesystem/process/network effects. It only decides whether the reviewed `steel.host.plan_turn` planning adapter may run for the current turn.

## Authority Vocabulary

Define a stable planning ability and resource shape:

- Ability: `clankers/steel.turn.plan`.
- Seam: `steel.host.plan_turn`.
- Resource: normalized logical URI such as `clankers://session/<session-id>/turn-planning/<profile-name>` or a deliberately hashed/redacted equivalent when the session identifier is not safe to display.
- Caveat classes: profile hash, script hash, rollout stage, fallback mode, target resource, expiry/not-before, replay/nonce where available, redaction class, and maximum planning input/output budget.

The vocabulary is planning-specific. It must not be confused with provider, tool, filesystem, shell, or mutation abilities.

## Rust Adapter Seam

Add a small adapter near the agent turn-planning activation boundary. The adapter should consume a DTO such as `SteelTurnPlanningAuthorityRequest` and return `SteelTurnPlanningAuthorityDecision`.

Inputs should be hashable and bounded:

- reviewed profile identity and BLAKE3 hash,
- reviewed script identity and BLAKE3 hash,
- normalized seam name,
- rollout/fallback settings,
- session identifier or redacted session reference,
- requested UCAN ability/resource,
- safe proof references or host-owned verifier handle,
- budget/caveat facts needed by the verifier.

The adapter should call the existing/sibling UCAN public API through a narrow Clankers adapter seam rather than reimplementing token parsing, proof traversal, revocation, replay, or attenuation semantics.

## Evaluation Order

1. Disabled/no-config remains disabled and does not require a UCAN planning receipt.
2. Settings/profile/script path/hash/budget validation runs first and fails closed as it does today.
3. Rust builds the normalized UCAN planning request.
4. Rust asks the UCAN adapter/verifier for an invocation decision.
5. Denied/missing/unavailable authority returns a structured activation error before Steel runs.
6. Allowed authority permits the existing `steel.host.plan_turn` adapter to run.
7. Rust still parses typed Steel plans, applies fallback/blocking policy, and owns all provider/tool/session effects.

## Receipts

Authority receipts must be deterministic and redacted. They may include:

- seam and stable ability,
- normalized/redacted resource reference,
- allowed/denied status,
- denial class,
- safe issuer/audience/proof-chain hash/reference,
- profile/script hashes,
- caveat class IDs,
- replay/revocation status where applicable,
- receipt hash.

Receipts must exclude raw compact UCAN tokens, signing keys, headers, environment values, prompts, provider payloads, raw profile/script bodies, and secret-bearing caveat values.

## Tests and Checker

Focused implementation should include:

- pure authority adapter tests for allowed, missing, expired, revoked, wrong audience/resource/ability, and overbroad/wrong caveat decisions;
- integration-style agent turn activation tests proving denied authority blocks before Steel/provider calls;
- a positive real-session/controller-style smoke proving allowed UCAN authority still reaches the Rust-owned provider path and emits both authority and Steel planning receipt markers;
- checker `scripts/check-steel-turn-planning-ucan-authority.rs` writing `target/steel-turn-planning-ucan-authority/receipt.json`.

## Seams Preserved

Steel remains a constrained trusted orchestration/planning language. It cannot gain ambient authority from UCAN, settings, profile data, script contents, or fallback policy. UCAN grants only the right for Rust to invoke the reviewed planning seam for a specific context. Rust remains the only component that performs I/O, provider calls, tool execution, mutation, fallback, rollback, receipts, and enforcement.
