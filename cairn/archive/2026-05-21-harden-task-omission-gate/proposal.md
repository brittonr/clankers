# Proposal: Harden task omission gate

## Problem

Review metrics still show repeated task-stage omission churn after migration to Cairn. The existing project-local review-gate rail catches broad deterministic contract omissions, but several high-frequency examples are narrower than the current categories: default/override request-body rules, active-account persistence, entitlement-probe retry variants, and raw tool-call delta stream boundaries.

## Proposed Change

Add a narrow Cairn package that strengthens the Clankers review-gate drift rail with sanitized fixtures and diagnostics for those repeated omission classes. Keep the scope limited to deterministic authoring/gate guidance and fixture-backed checks; do not change provider runtime behavior.

## Impact

- Future task ledgers must trace exact subcontracts to concrete fixture/helper/command/evidence tasks.
- Generic “request shape” or “SSE tests” tasks are no longer enough when proposal/design/spec text names repeated high-risk subcontracts.
- The change remains credential-free and uses sanitized examples only.
