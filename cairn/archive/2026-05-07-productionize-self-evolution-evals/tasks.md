## Phase 1: Spec Foundation

- [x] Write proposal, design, tasks, and delta spec for self-evolution production eval gates.
- [x] Validate the OpenSpec package with `openspec validate productionize-self-evolution-evals --strict` and record follow-up findings.

## Phase 2: Implementation

- [x] Inventory current self-evolution run/approve/apply/rollback, batch eval, and daemon/session-control seams; record exact integration points in `verification.md`.
- [x] Add eval corpus manifest models and parsers with positive/negative fixture tests.
- [x] Add objective scoring and regression-budget evaluation over local deterministic fixtures.
- [x] Route controlled-dogfood evaluation work through daemon/session events and record safe event receipts.
- [x] Add readiness report generation with `dry_run_only`, `controlled_dogfood`, `promotion_eligible`, and `blocked` states.
- [x] Tighten promotion recommendation logic to require corpus evidence, unchanged-candidate control, threshold pass, regression budget pass, and human approval readiness.
- [x] Update README and reference docs with the productionization profiles and anti-overclaiming guidance.

## Phase 3: Verification and Closeout

- [x] Run focused self-evolution unit/integration tests for corpus parsing, scoring, unchanged candidates, failed evals, readiness reports, and receipt redaction.
- [x] Run a deterministic CLI smoke that evaluates a disposable candidate through the controlled-dogfood profile without mutating active artifacts.
- [x] Run `cargo check --tests`, `openspec validate productionize-self-evolution-evals --strict`, and `git diff --check`.
- [x] Sync the delta spec into canonical specs and archive the change after implementation tasks complete.
