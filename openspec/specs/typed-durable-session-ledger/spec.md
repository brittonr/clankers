# typed-durable-session-ledger Specification

## Purpose
Define a structured, versioned, redacted session ledger that preserves legacy replay while enabling local typed queries over safe execution, artifact, OpenSpec, and work-tracking facts.
## Requirements
### Requirement: Typed durable session ledger

The system MUST persist structured, versioned session facts for agent execution while preserving existing human-readable transcript and JSONL compatibility. Authorization decisions for protected effects MUST be recorded as safe typed facts linked to effect, artifact, proof, caveat, replay, and revocation metadata.
r[typed-durable-session-ledger.records]

#### Scenario: typed facts are written for execution events
r[typed-durable-session-ledger.records.execution]

- GIVEN a session records user input, model request, model output, tool call, tool result, authorization decision, TUI block, review finding, or OpenSpec task event
- WHEN the session persistence layer records the event
- THEN it writes a typed ledger fact with record kind, schema version, safe identifiers, authorization status where applicable, and relevant artifact/proof hashes
- THEN it does not require downstream readers to infer these fields from rendered text

#### Scenario: raw secrets are not persisted as queryable facts
r[typed-durable-session-ledger.records.redaction]

- GIVEN an event contains credentials, headers, environment values, raw compact UCAN tokens, raw provider request bodies, or unredacted tool output
- WHEN the typed ledger fact is written
- THEN secret-bearing fields are omitted, redacted, or replaced with safe artifact/proof references according to the record schema

### Requirement: Legacy session compatibility

The system MUST keep existing session JSONL replay and export usable when typed ledger data is missing, partial, or newer than the reader.
r[typed-durable-session-ledger.compat]

#### Scenario: old session replays without ledger
r[typed-durable-session-ledger.compat.old-session]

- GIVEN an older session has JSONL transcript data but no typed ledger
- WHEN Clankers loads or replays the session
- THEN replay continues through the legacy path
- THEN missing typed facts are reported as unavailable metadata rather than a fatal error

### Requirement: Ledger schema migration

The system MUST provide explicit migration or safe fallback behavior for versioned ledger records.
r[typed-durable-session-ledger.migration]

#### Scenario: known old record migrates
r[typed-durable-session-ledger.migration.known-old]

- GIVEN a ledger contains a recognized old schema version
- WHEN Clankers opens the ledger with a newer reader
- THEN it migrates or projects the record into the current query shape using fixture-covered rules

#### Scenario: unknown future record is safe
r[typed-durable-session-ledger.migration.unknown-future]

- GIVEN a ledger contains a record kind or schema version newer than the reader supports
- WHEN Clankers opens the ledger
- THEN it preserves safe metadata and skips unsupported payload interpretation
- THEN replay of supported records continues

### Requirement: Structured pending-work ledger

The system MUST be able to record non-destructive pending refactor, OpenSpec, and repair work as structured ledger facts.
r[typed-durable-session-ledger.structured-work]

#### Scenario: pending work does not break current runnable state
r[typed-durable-session-ledger.structured-work.never-broken]

- GIVEN an agent identifies pending source edits, spec deltas, test repairs, or review findings
- WHEN it records the work in the structured ledger
- THEN existing session replay and current runnable source state remain valid
- THEN the pending work is represented as explicit todo facts with referenced files, requirements, artifact hashes, and verification status

### Requirement: Type and hash based session query

The system MUST support local queries over typed ledger facts by safe structured fields rather than only full-text transcript search.
r[typed-durable-session-ledger.query]

#### Scenario: query by artifact and execution shape
r[typed-durable-session-ledger.query.artifact-shape]

- GIVEN a session ledger contains artifact hashes, tool kinds, model request shapes, error classes, crate paths, and OpenSpec requirement IDs
- WHEN a caller queries by any supported field
- THEN Clankers returns matching sessions or events with redacted summaries and stable record identifiers
- THEN the query result excludes raw secrets and unredacted provider/tool payloads

#### Scenario: index rebuild is deterministic
r[typed-durable-session-ledger.query.rebuild]

- GIVEN a local query index is missing or stale
- WHEN Clankers rebuilds it from append-only typed ledger records
- THEN repeated rebuilds produce the same indexed facts for the same ledger contents

