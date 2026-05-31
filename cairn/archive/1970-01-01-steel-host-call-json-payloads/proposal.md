# Proposal: Steel Host-Call JSON Payloads

## Summary

Replace pipe-delimited Steel host-call payload strings for turn planning and execute-turn with typed JSON DTOs serialized by Rust-owned runtime types.

## Motivation

The current `steel.host.plan_turn` and `steel.host.execute_turn` fixture payloads are stable but brittle: field order is implicit, malformed fields collapse into generic parser failure, and downstream tests can only assert string markers. JSON DTOs keep the seam readable for receipts and docs while giving Rust explicit schema/version checks, typed hash fields, and clearer malformed-payload denial.

## Scope

- Add runtime-owned JSON payload DTOs for `steel.host.plan_turn` and `steel.host.execute_turn`.
- Serialize payloads with deterministic struct field order through Rust-owned helpers.
- Parse and validate JSON payloads instead of splitting on `|`.
- Update agent payload construction, docs, checker scripts, and focused tests.

## Non-Goals

- Introducing a binary codec or changing daemon protocol frames.
- Moving provider/tool effects into Steel.
- Replacing the constrained Steel runtime fixture evaluator.
