## ADDED Requirements

### Requirement: Current-HEAD payload binding [r[current-head-full-harness-receipt.payload-binding]]

Clankers MUST bind a full harness readiness receipt to the exact clean current HEAD payload commit being evaluated.

#### Scenario: Clean aligned payload is recorded
- GIVEN `main` is clean and aligned with `origin/main`
- WHEN `./scripts/test-harness.sh full` completes
- THEN `target/test-harness/results.json` MUST report the preflight HEAD as `payload.commit`
- AND `payload.tracked_dirty` MUST be false

### Requirement: Full harness pass evidence [r[current-head-full-harness-receipt.full-pass]]

A current-HEAD readiness refresh MUST require every full-harness step to pass before it is treated as readiness evidence.

#### Scenario: Full harness failures block readiness
- GIVEN the full harness writes results and summary artifacts
- WHEN either artifact reports failed steps
- THEN the refresh MUST be treated as blocked
- AND the failing receipt path MUST be preserved for the next repair slice

### Requirement: Generated receipt publication boundary [r[current-head-full-harness-receipt.non-publication]]

A current-HEAD harness refresh MUST NOT commit raw generated `target/` receipt artifacts by default.

#### Scenario: Raw receipt remains local
- GIVEN the harness writes logs under `target/test-harness`
- WHEN the receipt is used as operator evidence
- THEN only a later evidence-index slice MAY publish selected metadata
- AND raw generated logs MUST remain untracked unless explicitly requested
