## ADDED Requirements

### Requirement: Metrics-derived OpenSpec omission prevention [r[openspec-review-gates.metrics-derived-omission-prevention]]
The OpenSpec workflow MUST include a deterministic prevention rail for repeated review-metrics omission classes that occur often enough to justify mechanical checks or template rules.

#### Scenario: repeated task omission metrics become fixtures [r[openspec-review-gates.metrics-derived-omission-prevention.task-fixtures]]
- GIVEN review metrics identify a repeated task omission class with multiple representative examples
- WHEN the OpenSpec gate fixtures are updated
- THEN at least one sanitized fixture MUST encode the omitted contract and the incomplete task text
- AND the gate MUST fail that fixture with an actionable diagnostic before implementation tasks are marked complete

#### Scenario: metrics snapshot is safe and reviewable [r[openspec-review-gates.metrics-derived-omission-prevention.safe-snapshot]]
- GIVEN repeated review findings are used as input to a gate/template change
- WHEN the evidence is committed
- THEN the evidence MUST contain only sanitized finding summaries, counts, classes, and file/path examples safe for repository review
- AND it MUST NOT include credentials, private provider payloads, raw hidden prompts, or live secret material

### Requirement: Explicit deterministic verification tasks [r[openspec-review-gates.deterministic-verification-tasks]]
OpenSpec task ledgers MUST make deterministic verification obligations explicit when proposal, design, or specs require exact request shapes, stream boundaries, retry behavior, security policy, or fixture-backed evidence.

#### Scenario: vague test task misses a required deterministic contract [r[openspec-review-gates.deterministic-verification-tasks.vague-task]]
- GIVEN a spec or design requires an exact deterministic contract such as request headers, stream event boundaries, retry counts, redaction rules, or receipt fields
- WHEN `tasks.md` only says to test the feature generically
- THEN the gate MUST report that the deterministic contract is not traced to an explicit verification task
- AND the diagnostic SHOULD name the missing contract and the artifact that requires it

#### Scenario: concrete fixture task satisfies the contract [r[openspec-review-gates.deterministic-verification-tasks.fixture-task]]
- GIVEN a spec or design requires an exact deterministic contract
- WHEN `tasks.md` names a concrete fixture, helper, or command that verifies the contract and maps it with `[covers=...]`
- THEN the gate SHOULD treat the deterministic verification obligation as traceable

### Requirement: Oracle checkpoint escalation [r[openspec-review-gates.oracle-checkpoints]]
OpenSpec tasks MUST represent repeated human/oracle-routed review findings as explicit `H#` tasks with checked-in oracle-checkpoint evidence whenever mechanical checks cannot fully decide the issue.

#### Scenario: repeated human omission requires oracle evidence [r[openspec-review-gates.oracle-checkpoints.repeated-human-omission]]
- GIVEN a repeated review finding is routed to `human` because judgment or complete artifact context is required
- WHEN the task ledger claims the finding is resolved
- THEN an `H#` task MUST reference checked-in evidence under `openspec/changes/<change>/evidence/`
- AND the evidence MUST declare `Artifact-Type: oracle-checkpoint`, `Task-ID`, `Covers`, reviewed evidence, decision, and follow-up

#### Scenario: prose-only human resolution is rejected [r[openspec-review-gates.oracle-checkpoints.prose-only-rejected]]
- GIVEN a repeated human-routed finding has no `H#` task or oracle-checkpoint artifact
- WHEN the tasks gate runs
- THEN the gate MUST reject the closeout as unsupported prose rather than durable evidence
