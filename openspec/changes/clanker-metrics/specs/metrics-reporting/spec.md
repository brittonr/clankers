## ADDED Requirements

### Requirement: Users can query current-session and historical metrics
Clankers MUST expose current-session and historical metrics through a shared reporting surface used by standalone mode and attach mode.

#### Scenario: Standalone summary shows core session metrics
- **GIVEN** a local session has tool, plugin, and token activity
- **WHEN** the user runs `/metrics`
- **THEN** the report shows session totals, top tools, plugin activity counts, model split, token totals, estimated cost, and major latency summaries

#### Scenario: Historical report shows daily rollups
- **GIVEN** the metrics database contains data for prior days
- **WHEN** the user requests a historical metrics view
- **THEN** the report shows daily rollups with bounded per-model, per-tool, and per-plugin aggregates

### Requirement: Attach mode reports current-session and historical metrics
Clankers MUST let attached clients request the same session summary and historical rollup models that standalone mode uses.

#### Scenario: Attached client requests metrics for the active daemon session
- **GIVEN** a user is attached to a daemon-backed session
- **WHEN** the user requests session metrics
- **THEN** the daemon returns the shared session summary model for that session and the attach client renders it without recomputing aggregates locally

#### Scenario: Attached client requests historical metrics
- **GIVEN** a user is attached to a daemon-backed session and the metrics database contains prior-day rollups
- **WHEN** the user requests historical metrics from attach mode
- **THEN** the daemon returns the shared historical rollup model and the attach client renders the bounded per-day aggregates without recomputing them locally

#### Scenario: Plugin-heavy daemon session reports plugin counts
- **GIVEN** the active daemon session dispatched plugin events and plugin tools
- **WHEN** the attached client requests metrics
- **THEN** the returned summary includes plugin dispatch, hook-deny, UI-action, and plugin-tool counters for that session

### Requirement: Machine-readable metrics output is stable and versioned
Clankers MUST provide a machine-readable metrics representation with stable field names and an explicit version.

#### Scenario: JSON metrics output includes version and bounded structures
- **GIVEN** a metrics summary is serialized for automation or tests
- **WHEN** the machine-readable form is requested
- **THEN** the payload includes a version field, raw counters, histogram buckets, overflow counters, and digest fields needed to reproduce the human report

#### Scenario: Fingerprinted fields stay fingerprinted in reports
- **GIVEN** a machine-readable report includes hashed command or path dimensions
- **WHEN** the report is emitted
- **THEN** it exposes the digest and length metadata, not the original raw string
