## ADDED Requirements

### Requirement: Self-Evolution Eval Corpus Manifests [r[self-evolution-control.eval-corpus]]
Self-evolution MUST evaluate productionization candidates against explicit local corpus manifests before classifying a run as controlled-dogfood or promotion-eligible.

#### Scenario: Corpus manifest defines objective evidence [r[self-evolution-control.eval-corpus.scenario.manifest-evidence]]
- GIVEN a self-evolution run uses a productionization profile
- WHEN the run starts
- THEN it MUST load a local corpus manifest containing target identities, input cases or transcript refs, oracle/scoring commands, expected evidence outputs, and redaction policy

#### Scenario: Missing corpus blocks promotion readiness [r[self-evolution-control.eval-corpus.scenario.missing-blocks]]
- GIVEN a run has no accepted corpus manifest or the manifest fails validation
- WHEN the run produces a recommendation
- THEN the recommendation MUST be classified as `dry_run_only` or `blocked` and MUST NOT be `promotion_eligible`

### Requirement: Self-Evolution Controlled Dogfood Profile [r[self-evolution-control.controlled-dogfood]]
The system MUST run controlled self-evolution dogfood through the normal daemon/session substrate and record safe observable event receipts.

#### Scenario: Dogfood run is observable [r[self-evolution-control.controlled-dogfood.scenario.observable]]
- GIVEN a controlled-dogfood run dispatches agent work, tests, or review prompts
- WHEN the run executes
- THEN clankers MUST use daemon/session commands or the MCP session-control equivalent and record safe event counts, hashes, statuses, and interruption/completion receipts

#### Scenario: Hidden orchestration cannot be promoted [r[self-evolution-control.controlled-dogfood.scenario.hidden-blocked]]
- GIVEN candidate evaluation bypasses the daemon/session substrate with hidden local orchestration
- WHEN readiness is computed
- THEN the run MUST NOT be classified as `controlled_dogfood` or `promotion_eligible`

### Requirement: Self-Evolution Readiness Reporting [r[self-evolution-control.readiness-report]]
Self-evolution MUST emit a readiness report that distinguishes mechanical safety from productionization evidence.

#### Scenario: Report labels maturity [r[self-evolution-control.readiness-report.scenario.labels]]
- GIVEN a self-evolution run completes
- WHEN the report is generated
- THEN it MUST classify the run as one of `dry_run_only`, `controlled_dogfood`, `promotion_eligible`, or `blocked` with reasons, evidence refs, threshold outcomes, and known limitations

#### Scenario: Regression budget blocks promotion [r[self-evolution-control.readiness-report.scenario.regression-budget]]
- GIVEN a candidate improves one metric but fails a declared regression fixture, unchanged-candidate control, or minimum improvement threshold
- WHEN readiness is computed
- THEN the report MUST mark the run as not promotion-eligible and include safe failure evidence
