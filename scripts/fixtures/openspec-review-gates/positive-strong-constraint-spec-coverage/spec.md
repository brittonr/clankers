# lifecycle Specification

## Requirements

### Requirement: Strong lifecycle constraints are preserved
The lifecycle package MUST preserve generated artifact hygiene, local verification contract coverage, and the forbidden GitHub delivery path as normative delta spec behavior.

#### Scenario: generated artifact hygiene is traceable
- GIVEN generated artifacts are touched
- WHEN the lifecycle package is reviewed
- THEN generated artifact hygiene MUST be traced to evidence before closeout

#### Scenario: local verification preserves contract coverage
- GIVEN local verification is required by the proposal
- WHEN verification tasks close
- THEN local verification MUST prove contract coverage rather than optional generic evidence

#### Scenario: GitHub delivery path remains forbidden
- GIVEN the proposal defines a no-GitHub rule
- WHEN evidence is collected
- THEN the change MUST NOT use GitHub-only delivery as the source of lifecycle evidence
