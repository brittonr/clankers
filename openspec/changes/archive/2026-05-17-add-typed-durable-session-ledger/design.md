## Context

The ledger complements, rather than replaces, current session persistence. It stores structured facts derived at write time so future replay/search/review does not need to infer everything from display text.

## Goals / Non-Goals

**Goals:**
- Preserve durable typed facts for model requests, tool calls/results, TUI blocks, review findings, OpenSpec tasks, errors, and artifact hashes.
- Support schema version migrations and old-session replay.
- Provide precise query over typed fields.

**Non-Goals:**
- Replace human-readable transcripts.
- Store secrets or raw unredacted provider traffic.
- Require remote database services.

## Decision 1: Append typed facts beside JSONL

**Choice:** Session persistence writes an append-only typed ledger beside or within existing session storage, and JSONL export remains available.

**Rationale:** This preserves backward compatibility and lowers migration risk.

**Alternative:** Convert all sessions to a new DB-only format. Rejected because recovery and user tooling depend on text/session files.

## Decision 2: Versioned records with explicit migration fixtures

**Choice:** Each record kind carries a schema version. Migrations are explicit, fixture-backed, and may leave unrecognized records available as opaque safe metadata.

**Rationale:** Durable session storage must survive Clankers evolution.

## Decision 3: Structured change todo objects remain non-destructive

**Choice:** Pending refactor/spec/work items are stored as separate ledger facts referencing requirements/artifacts/files, not by mutating source state until implementation applies changes.

**Rationale:** This gives Unison-like never-broken work sessions while staying compatible with git/OpenSpec.

## Risks / Trade-offs

**Dual-write bugs** → Add parity tests between JSONL transcript and typed facts.

**Index drift** → Rebuild indexes from append-only ledger and verify counts.

**Privacy** → Store safe summaries and artifact hashes, not raw secrets.

## Validation Plan

- Old JSONL replay still works with no ledger.
- Ledger write/read round trips for each record kind.
- Migration fixtures for at least two record versions.
- Query tests by artifact hash, tool kind, error class, crate path, requirement ID, and model request shape.
