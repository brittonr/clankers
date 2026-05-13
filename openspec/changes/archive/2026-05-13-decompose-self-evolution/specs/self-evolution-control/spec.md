## ADDED Requirements

### Requirement: Self-Evolution Module Decomposition [r[self-evolution.decomposition]]

The self-evolution implementation MUST be split into safety-gate-oriented modules that preserve receipt-chain validation, dry-run/live-apply boundaries, and rollback guards.

#### Scenario: Receipt chain guards preserved [r[self-evolution.decomposition.scenario.1]]

- GIVEN a receipt, approval, target hash, candidate path, or application record is stale or mismatched
- WHEN a decomposed apply or rollback command runs
- THEN the operation fails before mutating target bytes, backups, or application/rollback receipts

#### Scenario: Dry-run boundary preserved [r[self-evolution.decomposition.scenario.2]]

- GIVEN a user runs preflight or dry-run apply/rollback
- WHEN the decomposed implementation evaluates the request
- THEN the target bytes and live backup/application/rollback state remain unchanged while planned receipt data is reported
