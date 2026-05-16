## Why

Clankers sessions are valuable executable history, but JSONL text and UI block replay are not enough for durable reasoning about schema evolution, structured refactors, todo repair, or type/hash-based search. Unison's typed durable storage and never-broken refactoring model map well to a Clankers ledger of typed session facts and pending change objects.

## What Changes

- **Typed session ledger**: Persist structured session facts alongside existing JSONL compatibility.
- **Schema/hash evolution**: Version ledger records and preserve old sessions across code changes.
- **Structured change todo ledger**: Represent pending refactor/spec/task work as explicit objects, not only prose.
- **Type/hash search**: Query sessions by typed facts such as tool kind, request shape, crate, error class, artifact hash, and requirement ID.

## Capabilities

### New Capabilities
- `typed-durable-session-ledger`: Structured, versioned session storage and query.

### Modified Capabilities
- `self-evolution-control`: May consume typed review/change facts.
- `thin-agent-shell`: Session shells may read/write typed facts through adapters.

## Impact

- **Files**: session crate, migration helpers, search/index modules, replay adapters, tests.
- **APIs**: typed ledger record enums and query API; existing JSONL remains supported.
- **Dependencies**: prefer embedded/local storage already used by Clankers where practical.
- **Testing**: migration fixtures, replay parity, query matrix, negative schema tests.
