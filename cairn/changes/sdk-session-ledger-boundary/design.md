# Design: Move Desktop Session Persistence Behind Neutral Ledger Boundaries

## Summary

Session storage is an application concern. The reusable SDK should accept host-owned session ledger entries and convert them to engine messages, while desktop Clankers can keep richer `AgentMessage` history behind adapters.

## Current coupling points

- `clankers-session` stores and restores `clanker_message::AgentMessage` values.
- Controller runtime adapters and conversion modules use `AgentMessage` for history replay.
- Root session setup/restore paths still use provider/message aliases and display block reconstruction.
- `clankers-runtime` has `SessionLedgerEntry` and examples proving product-owned session stores, but desktop paths are not fully behind that boundary.

## Decisions

### 1. Ledger DTOs are the SDK storage boundary

Embedding products should provide session ledger records or engine messages. They should not need Clankers session directories, automerge/JSONL details, DB search indexes, or desktop transcript IDs.

### 2. Desktop session store remains compatibility adapter

Existing history formats stay readable, but conversion to engine/semantic messages occurs at the desktop adapter edge.

### 3. Replay metadata parity remains required

Standalone restore and daemon attach replay must preserve user-message timestamps, finalized hashes, tool results, and compaction/branch context according to existing contracts.

## Validation plan

- Session storage inventory and selected restore/resume path migration.
- Runtime/session ledger fixture expansion.
- Desktop restore/attach replay parity tests.
- Dependency rails forbidding `clankers-session`/DB in green SDK examples and runtime kits unless explicitly app-edge.
