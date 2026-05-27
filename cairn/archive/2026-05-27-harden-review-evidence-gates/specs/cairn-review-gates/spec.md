## MODIFIED Requirements

### Requirement: Spec-stage omission prevention [r[cairn-review-gates.spec-stage-omission-prevention]]
The Cairn review-gate rail MUST reject repeated spec-stage omissions when proposal or design artifacts promise behavior that is not encoded as explicit delta spec requirements or scenarios.

#### Scenario: strong proposal constraint is missing or weakened in specs [r[cairn-review-gates.spec-stage-omission-prevention.strong-constraint-spec]]
- GIVEN a proposal or design artifact states a strong lifecycle constraint such as generated artifact hygiene, required local verification, forbidden delivery channels, source-preservation policy, or capability-boundary preservation
- WHEN the delta spec omits an equivalent normative requirement or weakens the constraint into optional, generic, or unrelated wording
- THEN the review-gate checker MUST report `missing-strong-constraint-spec`
- AND the diagnostic MUST name the source artifact and the constraint family that needs a delta spec requirement or scenario

#### Scenario: strong proposal constraint is preserved in specs [r[cairn-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied]]
- GIVEN a proposal or design artifact states a strong lifecycle constraint
- WHEN the delta spec includes explicit normative requirement or scenario text that preserves the same constraint strength and capability boundary
- THEN the review-gate checker SHOULD treat the constraint as traceable
- AND it SHOULD NOT emit `missing-strong-constraint-spec` for that constraint family
