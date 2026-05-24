## ADDED Requirements

### Requirement: Evidence index payload binding [r[current-head-release-evidence-index.index-current-payload]]

A release evidence index MUST identify the evaluated payload commit separately from any later evidence-recording commit.

#### Scenario: Payload is not self-referential
- GIVEN a docs page records current-HEAD evidence
- WHEN the page is committed after the harness run
- THEN it MUST name the evaluated payload commit without implying the docs commit was the evaluated payload

### Requirement: Receipt path references [r[current-head-release-evidence-index.index-receipt-paths]]

The evidence index MUST point to local generated receipt artifacts sufficient for operator inspection.

#### Scenario: Index links receipt artifacts
- GIVEN a full harness run produced results, summary, and logs
- WHEN the evidence index is reviewed
- THEN it MUST include the run id and receipt paths for `results.json`, `summary.md`, and relevant logs or log directory

### Requirement: Readiness tag boundary [r[current-head-release-evidence-index.tag-boundary]]

The evidence index MUST state whether readiness tags moved or remained unchanged.

#### Scenario: Tag boundary is explicit
- GIVEN current evidence is recorded after existing readiness tags
- WHEN operators read the index
- THEN the page MUST say whether the readiness tag was moved, not moved, or intentionally deferred

### Requirement: Index verification [r[current-head-release-evidence-index.index-verification]]

The evidence index MUST be checked against the generated receipt before closeout.

#### Scenario: Index matches receipt
- GIVEN the docs index states a run id and payload commit
- WHEN verification runs
- THEN the referenced receipt MUST exist and report the same payload commit and zero failures
