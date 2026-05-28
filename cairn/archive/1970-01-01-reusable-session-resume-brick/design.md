# Design: Reusable Session Resume Brick

## Summary

The session brick should own neutral replay semantics, not physical storage. It should define DTOs/traits for loading and saving prompt turns, assistant/tool content, summaries, usage, and receipt metadata. Backends may be in-memory, product-owned, JSONL, database, or desktop session stores.

## Decisions

### Decision: ledger entries are engine/session-neutral

Persisted entries should convert to `EngineMessage` and host-facing `SessionEvent` metadata without carrying `AgentMessage`, daemon frames, TUI blocks, or database row types as the reusable API.

### Decision: missing/unsupported stores fail closed

A runtime with no session store may run stateless prompts if configured, but resume must fail closed before model/tool execution when requested session state is absent or unsupported.

### Decision: desktop storage is an adapter

`clankers-session`/`clankers-db` can bridge to the ledger contract at the app edge. They should not become mandatory dependencies of generic SDK crates.

## Verification Plan

- Add in-memory and product fixture stores that round-trip restored context into `EngineModelRequest` order.
- Add desktop adapter parity for JSONL/database resume paths where applicable.
- Add boundary rails rejecting session DB/protocol/TUI types in reusable ledger APIs.
