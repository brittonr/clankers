# Design: Harden task omission gate

## Scope

The implementation extends `scripts/check-openspec-review-gates.rs`, its sanitized fixtures in `scripts/fixtures/openspec-review-gates/`, and `docs/src/reference/openspec-review-gates.md`.

## Decisions

1. Model repeated omission themes as deterministic contract categories, not as live-provider checks.
2. Require task text to name the specific subcontract and a concrete verification marker (`fixture`, `helper`, `command`, `golden`, `scripts/`, or `[evidence=...]`) with `[covers=...]`.
3. Add one negative sanitized Codex-shaped fixture proving generic request/SSE tasks do not satisfy exact subcontracts.
4. Add one positive sanitized fixture proving explicit fixture/helper/command tasks satisfy the same obligations.
5. Keep evidence secret-free: fixtures describe contracts and task text only; no provider payloads, tokens, account IDs, or live transcripts are committed.

## Verification

Run the Rust script rail directly, format-check Rust, build docs, validate/gate the Cairn change, and run `git diff --check`.
