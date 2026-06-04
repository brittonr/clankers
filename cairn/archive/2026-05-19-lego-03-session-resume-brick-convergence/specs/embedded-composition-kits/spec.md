## MODIFIED Requirements

### Requirement: Session/resume brick convergence [r[embedded-composition-kits.session-resume-brick]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Multiple product-shaped stores prove restored context [r[embedded-composition-kits.session-resume-brick.multiple-product-shaped-stores-prove-restored-context]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN At least two product-shaped session-store examples or fixtures prove that restored user/tool/assistant context reaches the next EngineModelRequest in deterministic order while storage DTOs remain product-owned.

#### Scenario: Missing and stale sessions fail closed [r[embedded-composition-kits.session-resume-brick.missing-and-stale-sessions-fail-closed]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Missing, stale, or fork-prone session ids produce explicit product-owned errors before model/tool execution unless the product takes a separate explicit new-session path.

#### Scenario: Reusable API promotion waits for convergence [r[embedded-composition-kits.session-resume-brick.reusable-api-promotion-waits-for-convergence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The change records repeated DTO/trait shapes as evidence for a later reusable API proposal instead of importing clankers-session, clankers-db, or shell JSONL state into green SDK crates.
