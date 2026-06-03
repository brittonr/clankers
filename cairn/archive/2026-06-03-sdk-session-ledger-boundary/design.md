# Design: Move Desktop Session Persistence Behind Neutral Ledger Boundaries

## Summary

Session storage is an application concern. The reusable SDK should accept host-owned session ledger entries and convert them to engine messages, while desktop Clankers can keep richer `AgentMessage` history behind adapters.

## Current coupling points

- `clankers-session` stores and restores `clanker_message::AgentMessage` values.
- Controller runtime adapters and conversion modules use `AgentMessage` for history replay.
- Root session setup/restore paths still use provider/message aliases and display block reconstruction.
- `clankers-runtime` has `SessionLedgerEntry` and examples proving product-owned session stores, but desktop paths are not fully behind that boundary.

## Phase 1 inventory

`scripts/check-session-ledger-boundary.rs` records the initial DTO owner map for the selected slice:

- `crates/clankers-runtime/src/{ledger,session}.rs`: neutral SDK ledger/session DTO boundary and resume runtime; forbidden from desktop session stores, DB/search, daemon events, and TUI replay DTOs.
- `examples/embedded-session-store` and `examples/embedded-product-workbench`: host-owned product session/message DTOs; forbidden from `AgentMessage`, `clankers-session`, Clankers DB, and global session directories.
- `src/modes/session_ledger.rs`: selected desktop transcript-to-neutral ledger adapter for the daemon socketless resume seed path.
- `src/modes/daemon/session_builder.rs`: selected restore/resume path now calls the neutral ledger adapter before emitting daemon seed protocol messages.
- `crates/clankers-session/src/{lib,merge}.rs`, `src/modes/{session_setup,interactive}.rs`, and `crates/clankers-controller/src/persistence.rs`: desktop compatibility setup/storage/merge/restore/persistence adapters that may still own `AgentMessage` and `SessionManager` usage.
- `src/modes/session_restore.rs`, `crates/clankers-controller/src/convert.rs`, and `src/modes/attach/events.rs`: display replay projection/app-edge paths that translate stored desktop records into TUI replay events.

This establishes the owner receipt and selects the daemon `SessionBuilder` resume seed path as the first migration target. That path still reads existing desktop session files through `clankers-session`, but conversion now happens at `src/modes/session_ledger.rs` before reusable seed/replay behavior consumes neutral ledger DTOs.

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
