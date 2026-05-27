## MODIFIED Requirements

### Requirement: Explicit deterministic verification tasks [r[cairn-review-gates.deterministic-verification-tasks]]
Cairn task ledgers MUST make deterministic verification obligations explicit when proposal, design, or specs require exact request shapes, stream boundaries, retry behavior, security policy, fixture-backed evidence, provider/account state, or exact default/override behavior.

#### Scenario: vague test task misses a required deterministic contract [r[cairn-review-gates.deterministic-verification-tasks.vague-task]]
- GIVEN a spec or design requires an exact deterministic contract such as request headers, stream event boundaries, retry counts, redaction rules, or receipt fields
- WHEN `tasks.md` only says to test the feature generically
- THEN the gate MUST report that the deterministic contract is not traced to an explicit verification task
- AND the diagnostic SHOULD name the missing contract and the artifact that requires it

#### Scenario: concrete fixture task satisfies the contract [r[cairn-review-gates.deterministic-verification-tasks.fixture-task]]
- GIVEN a spec or design requires an exact deterministic contract
- WHEN `tasks.md` names a concrete fixture, helper, or command that verifies the contract and maps it with `[covers=...]`
- THEN the gate SHOULD treat the deterministic verification obligation as traceable

#### Scenario: repeated subcontract omission is rejected [r[cairn-review-gates.deterministic-verification-tasks.repeated-subcontract-omission]]
- GIVEN a proposal, design, or spec requires exact subcontracts such as default/override request fields, active account persistence, entitlement probe retry behavior, or raw tool-call delta stream boundaries
- WHEN `tasks.md` only names a broad request-shape, auth, retry, or SSE test
- THEN the gate MUST report a subcontract-specific missing-task diagnostic
- AND the diagnostic MUST require an explicit fixture, helper, command, golden, script, or evidence-backed task for that subcontract

#### Scenario: concrete subcontract fixture task satisfies the obligation [r[cairn-review-gates.deterministic-verification-tasks.subcontract-fixture-task]]
- GIVEN a proposal, design, or spec requires one of the repeated high-risk subcontracts
- WHEN `tasks.md` names the subcontract and a concrete fixture, helper, command, golden, script, or evidence path with `[covers=...]`
- THEN the gate SHOULD treat the subcontract verification obligation as traceable
