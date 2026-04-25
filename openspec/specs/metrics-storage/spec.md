## ADDED Requirements

### Requirement: Metrics persist in redb as summaries, rollups, and bounded recent events
Clankers MUST persist metrics in redb using session summaries, daily rollups, and a bounded recent-event log.

#### Scenario: Session flush writes a summary
- **GIVEN** a session has recorded tool, plugin, and token activity
- **WHEN** the metrics writer flushes that session
- **THEN** redb stores a session summary keyed by session ID with aggregate counters, histograms, and retention metadata

#### Scenario: Daily rollup merges multiple sessions
- **GIVEN** two sessions finish on the same UTC date
- **WHEN** their metrics are flushed
- **THEN** the daily rollup for that date merges both sessions into one bounded aggregate record

#### Scenario: Recent events stay keyed by session order
- **GIVEN** a session emits multiple metric events
- **WHEN** the recent-event log is written
- **THEN** the stored keys preserve session-local ordering so the newest events can be queried without scanning unrelated sessions

### Requirement: High-cardinality dimensions use BLAKE3 fingerprints
Clankers MUST normalize and BLAKE3-fingerprint high-cardinality or sensitive strings before storing them in metrics tables.

#### Scenario: Bash command is fingerprinted
- **GIVEN** a `bash` tool call includes a command string
- **WHEN** metrics persistence stores tool dimensions
- **THEN** it stores the normalized command's BLAKE3 digest and byte length instead of the raw command text

#### Scenario: File path is fingerprinted
- **GIVEN** a `read`, `write`, or `edit` tool metric includes a filesystem path
- **WHEN** metrics persistence stores the path dimension
- **THEN** it stores the normalized path digest and path length instead of the raw path string

#### Scenario: Low-cardinality labels remain readable
- **GIVEN** a metrics summary includes model, tool, and plugin names
- **WHEN** the summary is serialized for reporting
- **THEN** those low-cardinality labels remain plain strings rather than opaque digests

### Requirement: Metrics retention and cardinality are explicitly bounded
Clankers MUST bound raw-event retention and aggregate cardinality with fixed limits and overflow counters.

#### Scenario: Recent-event cap is reached
- **GIVEN** a session has reached the configured recent-event retention limit
- **WHEN** more metric events arrive
- **THEN** the oldest stored events are dropped in batches and the session summary increments `recent_events_dropped`

#### Scenario: Heavy-hitter cap is reached
- **GIVEN** more distinct digests or labels appear than a summary's top-N capacity allows
- **WHEN** the reducer updates that summary
- **THEN** excess entries are merged into an `other` bucket instead of allocating unbounded state

#### Scenario: Histogram storage stays fixed-size
- **GIVEN** tool and plugin latencies span a wide range
- **WHEN** those samples are folded into metrics storage
- **THEN** they are recorded in fixed histogram buckets rather than an unbounded vector of raw samples

### Requirement: Metrics persistence is best-effort
Clankers MUST treat metrics persistence as best-effort and MUST NOT fail user work when a metrics write fails.

#### Scenario: redb write fails during flush
- **GIVEN** the metrics writer hits a redb error while flushing a session update
- **WHEN** the failure is detected
- **THEN** the runtime logs a warning, increments a dropped-write counter, and continues the user session without aborting the tool or turn
