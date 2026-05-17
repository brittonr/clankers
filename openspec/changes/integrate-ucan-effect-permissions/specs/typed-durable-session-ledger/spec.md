## MODIFIED Requirements

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
