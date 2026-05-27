## MODIFIED Requirements

### Requirement: Explicit deterministic verification tasks [r[cairn-review-gates.deterministic-verification-tasks]]
Cairn task ledgers MUST make deterministic verification obligations explicit when proposal, design, or specs require exact request shapes, stream boundaries, retry behavior, security policy, fixture-backed evidence, provider/account state, exact default/override behavior, or deterministic check coverage.

#### Scenario: deterministic-check task omits the reproducible artifact [r[cairn-review-gates.deterministic-verification-tasks.deterministic-check-artifact]]
- GIVEN a proposal, design, or spec requires deterministic check coverage or fixture-backed verification
- WHEN `tasks.md` only says to add tests, verify deterministic behavior, or ensure fixture coverage without naming a concrete artifact
- THEN the gate MUST report `missing-deterministic-check-artifact-task`
- AND the diagnostic MUST require a task line with `[covers=...]` plus a concrete fixture, helper, command, golden file, script, evidence path, or oracle checkpoint

#### Scenario: concrete deterministic-check artifact task satisfies the obligation [r[cairn-review-gates.deterministic-verification-tasks.deterministic-check-artifact-satisfied]]
- GIVEN a proposal, design, or spec requires deterministic check coverage or fixture-backed verification
- WHEN `tasks.md` names the deterministic check and a concrete fixture, helper, command, golden file, script, evidence path, or oracle checkpoint with `[covers=...]`
- THEN the gate SHOULD treat the deterministic-check obligation as traceable
