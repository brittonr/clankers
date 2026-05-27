# openspec-review-gates Specification

## Purpose
TBD - created by archiving change roi-01-harden-openspec-gate-omission-prevention. Update Purpose after archive.

## Requirements

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

### Requirement: Explicit deterministic verification tasks [r[openspec-review-gates.deterministic-verification-tasks]]
OpenSpec task ledgers MUST make deterministic verification obligations explicit when proposal, design, or specs require exact request shapes, stream boundaries, retry behavior, security policy, fixture-backed evidence, provider/account state, exact default/override behavior, or deterministic check coverage.

#### Scenario: vague test task misses a required deterministic contract [r[openspec-review-gates.deterministic-verification-tasks.vague-task]]
- GIVEN a spec or design requires an exact deterministic contract such as request headers, stream event boundaries, retry counts, redaction rules, or receipt fields
- WHEN `tasks.md` only says to test the feature generically
- THEN the gate MUST report that the deterministic contract is not traced to an explicit verification task
- AND the diagnostic SHOULD name the missing contract and the artifact that requires it

#### Scenario: concrete fixture task satisfies the contract [r[openspec-review-gates.deterministic-verification-tasks.fixture-task]]
- GIVEN a spec or design requires an exact deterministic contract
- WHEN `tasks.md` names a concrete fixture, helper, or command that verifies the contract and maps it with `[covers=...]`
- THEN the gate SHOULD treat the deterministic verification obligation as traceable

#### Scenario: repeated subcontract omission is rejected [r[openspec-review-gates.deterministic-verification-tasks.repeated-subcontract-omission]]
- GIVEN a proposal, design, or spec requires exact subcontracts such as default/override request fields, active account persistence, entitlement probe retry behavior, or raw tool-call delta stream boundaries
- WHEN `tasks.md` only names a broad request-shape, auth, retry, or SSE test
- THEN the gate MUST report a subcontract-specific missing-task diagnostic
- AND the diagnostic MUST require an explicit fixture, helper, command, golden, script, or evidence-backed task for that subcontract

#### Scenario: concrete subcontract fixture task satisfies the obligation [r[openspec-review-gates.deterministic-verification-tasks.subcontract-fixture-task]]
- GIVEN a proposal, design, or spec requires one of the repeated high-risk subcontracts
- WHEN `tasks.md` names the subcontract and a concrete fixture, helper, command, golden, script, or evidence path with `[covers=...]`
- THEN the gate SHOULD treat the subcontract verification obligation as traceable

#### Scenario: deterministic-check task omits the reproducible artifact [r[openspec-review-gates.deterministic-verification-tasks.deterministic-check-artifact]]
- GIVEN a proposal, design, or spec requires deterministic check coverage or fixture-backed verification
- WHEN `tasks.md` only says to add tests, verify deterministic behavior, or ensure fixture coverage without naming a concrete artifact
- THEN the gate MUST report `missing-deterministic-check-artifact-task`
- AND the diagnostic MUST require a task line with `[covers=...]` plus a concrete fixture, helper, command, golden file, script, evidence path, or oracle checkpoint

#### Scenario: concrete deterministic-check artifact task satisfies the obligation [r[openspec-review-gates.deterministic-verification-tasks.deterministic-check-artifact-satisfied]]
- GIVEN a proposal, design, or spec requires deterministic check coverage or fixture-backed verification
- WHEN `tasks.md` names the deterministic check and a concrete fixture, helper, command, golden file, script, evidence path, or oracle checkpoint with `[covers=...]`
- THEN the gate SHOULD treat the deterministic-check obligation as traceable

### Requirement: Design-stage omission prevention [r[openspec-review-gates.design-stage-omission-prevention]]
The OpenSpec review-gate rail MUST reject repeated design-stage omissions when proposal or delta spec artifacts require concrete design behavior but `design.md` leaves the storage seam, policy bounds, or verification plan ambiguous.

#### Scenario: repeated design omissions become deterministic diagnostics [r[openspec-review-gates.design-stage-omission-prevention.design-categories]]
- GIVEN proposal or delta spec artifacts require reasoning signature retention, retry-policy bounds, or scenario-complete verification planning
- WHEN the design review gate evaluates the change artifacts
- THEN it MUST report category-specific diagnostics for missing concrete design details
- AND the diagnostics MUST be deterministic, sanitized, and actionable without live provider credentials

#### Scenario: vague design fixture is rejected [r[openspec-review-gates.design-stage-omission-prevention.design-fixtures]]
- GIVEN a sanitized fixture whose proposal requires reasoning signature retention, retry bounds, and a complete verification plan
- WHEN `design.md` only says to map reasoning, implement retry support, and cover parsing with recorded fixtures
- THEN the review-gate fixture runner MUST reject the fixture with `missing-reasoning-signature-design`, `missing-retry-policy-design`, and `missing-verification-plan-design`

#### Scenario: concrete design fixture satisfies the gate [r[openspec-review-gates.design-stage-omission-prevention.design-docs]]
- GIVEN a sanitized fixture whose proposal requires the same design obligations
- WHEN `design.md` names storage/reuse for later-turn reasoning signatures, exact retry/backoff/refresh bounds, and scenario-complete verification cases
- THEN the review-gate fixture runner SHOULD pass the fixture
- AND the operator documentation MUST list the new design-stage diagnostics

### Requirement: Metrics-derived OpenSpec omission prevention [r[openspec-review-gates.review-metrics-regression-rail]]
The OpenSpec review-gate workflow MUST maintain a deterministic regression loop from repeated sanitized review metrics to fixture-backed diagnostics and operator guidance.

#### Scenario: metrics snapshot selects the next fixture category [r[openspec-review-gates.review-metrics-regression-rail.snapshot-selects-category]]
- GIVEN review metrics identify repeated omission or incoherence findings above the project threshold
- WHEN a review-gate hardening change is planned
- THEN the change MUST include a sanitized evidence snapshot with counts, classes, source stages, and representative safe examples
- AND the snapshot MUST identify which category is selected for deterministic fixture hardening

#### Scenario: fixture-backed metrics category is enforced [r[openspec-review-gates.review-metrics-regression-rail.fixture-backed-category]]
- GIVEN a metrics-selected category is not already covered by the checker
- WHEN the review-gate fixtures are updated
- THEN at least one negative fixture MUST fail with a category-specific diagnostic
- AND at least one positive fixture MUST pass by naming the concrete fixture, helper, command, evidence path, or oracle checkpoint that satisfies the obligation

#### Scenario: metrics evidence remains secret-free [r[openspec-review-gates.review-metrics-regression-rail.secret-free-evidence]]
- GIVEN repeated review findings are summarized as checked-in evidence
- WHEN the evidence is committed
- THEN it MUST NOT include credentials, tokens, account identifiers, raw hidden prompts, provider payload bodies, or private transcript dumps
- AND it MUST preserve enough sanitized context for a reviewer to understand the prevention rail being added

#### Scenario: guidance and wiring stay aligned [r[openspec-review-gates.review-metrics-regression-rail.guidance-and-wiring]]
- GIVEN a new metrics-derived diagnostic or fixture category is added
- WHEN the focused review-gate checker runs
- THEN operator guidance MUST document the diagnostic and expected artifact/task shape
- AND the repo check wiring MUST continue to run the focused checker as part of the maintained rails

### Requirement: Spec-stage omission prevention [r[openspec-review-gates.spec-stage-omission-prevention]]
The OpenSpec review-gate rail MUST reject repeated spec-stage omissions when proposal or design artifacts promise behavior that is not encoded as explicit delta spec requirements or scenarios.

#### Scenario: strong proposal constraint is missing or weakened in specs [r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec]]
- GIVEN a proposal or design artifact states a strong lifecycle constraint such as generated artifact hygiene, required local verification, forbidden delivery channels, source-preservation policy, or capability-boundary preservation
- WHEN the delta spec omits an equivalent normative requirement or weakens the constraint into optional, generic, or unrelated wording
- THEN the review-gate checker MUST report `missing-strong-constraint-spec`
- AND the diagnostic MUST name the source artifact and the constraint family that needs a delta spec requirement or scenario

#### Scenario: strong proposal constraint is preserved in specs [r[openspec-review-gates.spec-stage-omission-prevention.strong-constraint-spec-satisfied]]
- GIVEN a proposal or design artifact states a strong lifecycle constraint
- WHEN the delta spec includes explicit normative requirement or scenario text that preserves the same constraint strength and capability boundary
- THEN the review-gate checker SHOULD treat the constraint as traceable
- AND it SHOULD NOT emit `missing-strong-constraint-spec` for that constraint family
