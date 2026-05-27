# cairn-review-gates Delta

## MODIFIED Requirements

### Requirement: Metrics-derived Cairn omission prevention [r[cairn-review-gates.review-metrics-regression-rail]]
The Cairn review-gate workflow MUST maintain a deterministic regression loop from repeated sanitized review metrics to fixture-backed diagnostics and operator guidance.

#### Scenario: metrics snapshot selects the next fixture category [r[cairn-review-gates.review-metrics-regression-rail.snapshot-selects-category]]
- GIVEN review metrics identify repeated omission or incoherence findings above the project threshold
- WHEN a review-gate hardening change is planned
- THEN the change MUST include a sanitized evidence snapshot with counts, classes, source stages, and representative safe examples
- AND the snapshot MUST identify which category is selected for deterministic fixture hardening

#### Scenario: fixture-backed metrics category is enforced [r[cairn-review-gates.review-metrics-regression-rail.fixture-backed-category]]
- GIVEN a metrics-selected category is not already covered by the checker
- WHEN the review-gate fixtures are updated
- THEN at least one negative fixture MUST fail with a category-specific diagnostic
- AND at least one positive fixture MUST pass by naming the concrete fixture, helper, command, evidence path, or oracle checkpoint that satisfies the obligation

#### Scenario: metrics evidence remains secret-free [r[cairn-review-gates.review-metrics-regression-rail.secret-free-evidence]]
- GIVEN repeated review findings are summarized as checked-in evidence
- WHEN the evidence is committed
- THEN it MUST NOT include credentials, tokens, account identifiers, raw hidden prompts, provider payload bodies, or private transcript dumps
- AND it MUST preserve enough sanitized context for a reviewer to understand the prevention rail being added

#### Scenario: guidance and wiring stay aligned [r[cairn-review-gates.review-metrics-regression-rail.guidance-and-wiring]]
- GIVEN a new metrics-derived diagnostic or fixture category is added
- WHEN the focused review-gate checker runs
- THEN operator guidance MUST document the diagnostic and expected artifact/task shape
- AND the repo check wiring MUST continue to run the focused checker as part of the maintained rails
