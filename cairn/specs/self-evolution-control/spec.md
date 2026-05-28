# self-evolution-control Specification

## Requirements

### Requirement: Self-evolution receipt chain kit proves gated artifact promotion

The self-evolution-receipt-chain-kit SHALL validate run receipt, approval receipt, application receipt, and rollback receipt order before mutation.

#### Scenario: receipt chain is explicit
- GIVEN an artifact promotion is requested
- WHEN approval and application receipts are checked
- THEN run receipt, approval receipt, application receipt, and rollback receipt data MUST be linked explicitly.

#### Scenario: fail-closed chain guards
- GIVEN stale target hash, mismatched approval, missing candidate artifact, unsupported apply mode, or already-applied approval is detected
- WHEN validation runs
- THEN application MUST fail closed before mutation.
