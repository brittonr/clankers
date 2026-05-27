# cairn-review-gates Specification

## Requirements

### Requirement: Design-stage omission prevention [r[cairn-review-gates.design-stage-omission-prevention]]
The Cairn review-gate rail MUST reject repeated design-stage omissions when proposal or delta spec artifacts require concrete design behavior but `design.md` leaves the storage seam, policy bounds, or verification plan ambiguous.

#### Scenario: repeated design omissions become deterministic diagnostics [r[cairn-review-gates.design-stage-omission-prevention.design-categories]]
- GIVEN proposal or delta spec artifacts require reasoning signature retention, retry-policy bounds, or scenario-complete verification planning
- WHEN the design review gate evaluates the change artifacts
- THEN it MUST report category-specific diagnostics for missing concrete design details
- AND the diagnostics MUST be deterministic, sanitized, and actionable without live provider credentials

#### Scenario: vague design fixture is rejected [r[cairn-review-gates.design-stage-omission-prevention.design-fixtures]]
- GIVEN a sanitized fixture whose proposal requires reasoning signature retention, retry bounds, and a complete verification plan
- WHEN `design.md` only says to map reasoning, implement retry support, and cover parsing with recorded fixtures
- THEN the review-gate fixture runner MUST reject the fixture with `missing-reasoning-signature-design`, `missing-retry-policy-design`, and `missing-verification-plan-design`

#### Scenario: concrete design fixture satisfies the gate [r[cairn-review-gates.design-stage-omission-prevention.design-docs]]
- GIVEN a sanitized fixture whose proposal requires the same design obligations
- WHEN `design.md` names storage/reuse for later-turn reasoning signatures, exact retry/backoff/refresh bounds, and scenario-complete verification cases
- THEN the review-gate fixture runner SHOULD pass the fixture
- AND the operator documentation MUST list the new design-stage diagnostics
