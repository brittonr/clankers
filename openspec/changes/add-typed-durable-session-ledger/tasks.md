## Phase 1: Ledger model

- [x] [serial] Define typed ledger record kinds, schema versions, redaction rules, and opaque-unknown fallback. [covers=typed-durable-session-ledger.records] ✅ 2m 11s (started: 2026-05-17T00:00:37Z → completed: 2026-05-17T00:02:48Z; evidence: `cargo test -p clankers-session ledger -- --nocapture`; `cargo test -p clankers-session`)
- [ ] [depends:ledger-records] Add append/read round-trip tests for model, tool, block, review, OpenSpec, error, and artifact-reference records. [covers=typed-durable-session-ledger.records]

## Phase 2: Persistence and migration

- [ ] [depends:ledger-records] Write typed ledger facts alongside existing session JSONL without breaking legacy replay. [covers=typed-durable-session-ledger.compat]
- [ ] [depends:ledger-write] Add schema migration fixtures and old-session compatibility tests. [covers=typed-durable-session-ledger.migration]
- [ ] [depends:ledger-write] Add structured pending-change/todo ledger records for non-destructive refactor and OpenSpec work sessions. [covers=typed-durable-session-ledger.structured-work]

## Phase 3: Query and verification

- [ ] [depends:ledger-write] Build a local index/query API for artifact hash, tool kind, error class, crate path, requirement ID, and request shape. [covers=typed-durable-session-ledger.query]
- [ ] [depends:ledger-query] Add CLI or internal inspection path with redacted query results and missing-index rebuild behavior. [covers=typed-durable-session-ledger.inspect]
- [ ] [serial] Run focused ledger/migration/query tests and a replay parity subset. [covers=typed-durable-session-ledger.validation]
