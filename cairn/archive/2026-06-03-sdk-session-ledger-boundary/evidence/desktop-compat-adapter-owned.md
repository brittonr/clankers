Task-ID: I3
Covers: sdk-session-ledger-boundary.desktop-compat.adapter-owned
Artifact-Type: boundary-evidence

# Desktop Compatibility Adapter Ownership

## Summary

The boundary rail now separates desktop compatibility owners from neutral SDK/runtime owners:

- `crates/clankers-session/src/lib.rs` owns existing automerge/legacy JSONL `SessionManager` storage and `AgentMessage` append/read behavior.
- `crates/clankers-session/src/merge.rs` owns branch merge/cherry-pick storage behavior that still clones desktop `AgentMessage` records.
- `src/modes/session_setup.rs` and `src/modes/interactive.rs` remain desktop setup/restore shells for existing session files.
- `src/modes/session_ledger.rs` is the app-edge adapter that projects selected desktop transcript records into neutral `SessionLedgerEntry` DTOs.
- `crates/clankers-controller/src/persistence.rs` owns controller persistence and DB search indexing for desktop sessions.
- Display replay stays in `crates/clankers-controller/src/convert.rs` and `src/modes/attach/events.rs`.

## Boundary rail

`scripts/check-session-ledger-boundary.rs` forbids `clankers_session`, `SessionManager`, DB/search, daemon, and TUI DTO markers in the neutral runtime ledger/resume paths, and separately records the desktop adapter owners where those compatibility types remain allowed.
