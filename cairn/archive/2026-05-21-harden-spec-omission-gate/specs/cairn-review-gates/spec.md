# cairn-review-gates Specification

## Requirements

### Requirement: Spec-stage omission prevention [r[cairn-review-gates.spec-stage-omission-prevention]]
The Cairn review-gate rail MUST reject repeated spec-stage omissions when proposal or design artifacts promise behavior that is not encoded as explicit delta spec requirements or scenarios.

#### Scenario: repeated spec omissions become deterministic diagnostics [r[cairn-review-gates.spec-stage-omission-prevention.spec-categories]]
- GIVEN proposal or design artifacts promise omitted-provider defaults, malformed account-claim handling, or provider-scoped status behavior
- WHEN the spec review gate evaluates the change artifacts
- THEN it MUST report category-specific diagnostics for missing delta spec requirements or scenarios
- AND the diagnostics MUST be deterministic, sanitized, and actionable without live provider credentials

#### Scenario: vague spec fixture is rejected [r[cairn-review-gates.spec-stage-omission-prevention.spec-fixtures]]
- GIVEN a sanitized fixture whose proposal and design require omitted-provider defaults, malformed `chatgpt_account_id` claim handling, and provider-scoped status behavior
- WHEN `spec.md` only contains a broad provider-login requirement
- THEN the review-gate fixture runner MUST reject the fixture with `missing-omitted-provider-default-spec`, `missing-malformed-account-claim-spec`, and `missing-provider-scoped-status-spec`

#### Scenario: concrete spec fixture satisfies the gate [r[cairn-review-gates.spec-stage-omission-prevention.spec-docs]]
- GIVEN a sanitized fixture whose proposal and design require the same spec obligations
- WHEN `spec.md` contains explicit scenarios for omitted provider Anthropic defaults, malformed claim handling, and provider-scoped `openai-codex` status
- THEN the review-gate fixture runner SHOULD pass the fixture
- AND the operator documentation MUST list the new spec-stage diagnostics
