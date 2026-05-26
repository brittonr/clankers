# ADDED Requirements

### Requirement: rail package [r[review-metrics-regression-rail.metrics-snapshot]]

The rail package MUST include a sanitized metrics snapshot for the selected repeated review finding classes.

#### Scenario: Sanitized metrics evidence is preserved
- GIVEN review metrics are collected for planning
- WHEN the change is reviewed
- THEN the package includes counts, keys, sources, stages, and behavior summaries without secrets or raw private transcripts

### Requirement: checker [r[review-metrics-regression-rail.fixture-regression]]

The checker MUST include deterministic fixtures for repeated omission categories before generic gate changes are considered.

#### Scenario: Repeated omissions become fixtures
- GIVEN the top repeated omission category is selected
- WHEN the checker runs
- THEN it reports a stable issue for a fixture that omits required task verification detail

### Requirement: first implementation [r[review-metrics-regression-rail.project-local-first]]

The first implementation MUST harden Clankers repo-local review-gate fixtures/docs before touching generic Cairn/OpenSpec core.

#### Scenario: Repo-local rail owns first prevention
- GIVEN a repeated Clankers finding category is unsupported
- WHEN implementation starts
- THEN the Clankers checker and docs are updated before any shared lifecycle engine changes

### Requirement: rail [r[review-metrics-regression-rail.deterministic-report]]

The rail MUST produce deterministic pass/fail output suitable for metrics promotion tracking.

#### Scenario: Deterministic report is stable
- GIVEN the same fixtures are checked twice
- WHEN the rail runs in both invocations
- THEN the issue codes, counts, and fixture identifiers are stable
