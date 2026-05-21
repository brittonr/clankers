# Design: harden design omission gate

## Scope

This is a deterministic review-gate hardening slice. The gate reads sanitized fixtures under `scripts/fixtures/openspec-review-gates/` and never calls provider APIs or reads credentials.

## Decisions

1. Add a `DesignCategory` table beside the existing task `ContractCategory` table.
2. Trigger design diagnostics from `proposal.md` and delta `spec.md` text, not from `design.md` itself, so incomplete design prose cannot satisfy its own obligation.
3. Require concrete terms for the repeated omission classes seen in metrics:
   - reasoning signature retention requires storage, reuse, and later-turn behavior;
   - retry policy requires 3 retries, `1s/2s/4s` backoff, exactly one 401 refresh retry, and one refresh cycle;
   - verification plan requires proactive refresh, 401, 429, provider-scoped status, and discovery hiding cases.
4. Keep fixtures sanitized and credential-free.

## Verification Plan

- Negative fixture: proposal requires the three design obligations but design uses broad prose; checker emits three missing-design diagnostics.
- Positive fixture: design names the required storage/policy/verification details; checker passes.
- Existing task/oracle fixtures keep passing.
