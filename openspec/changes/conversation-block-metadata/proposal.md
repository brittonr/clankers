## Why

Clankers renders conversation history as prompt-and-response blocks, but block metadata is still ephemeral. Live blocks get a local creation time, restored blocks are rebuilt with fresh timestamps, and there is no stable content-derived identifier for a block across replay, attach mode, or other in-process/machine-readable block consumers.

## What Changes

- Add canonical per-block timestamps derived from persisted conversation messages instead of transient TUI construction time.
- Add finalized per-block BLAKE3 hashes so each conversation block has a stable content-derived identity across restore, attach, and other in-process/machine-readable block consumers.
- Expose block metadata through the shared block model used by live rendering, history replay, and any machine-readable block surfaces.
- Keep block metadata derivation deterministic and replay-safe: same persisted message sequence yields the same timestamp and hash.

## Capabilities

### New Capabilities
- `conversation-block-metadata`: Canonical timestamp and BLAKE3 hash metadata for each conversation block, preserved across live sessions and restored history.

### Modified Capabilities
None. I checked `openspec/specs/` and there is no existing base capability for conversation-block metadata.

## Impact

- `crates/clankers-tui-types/src/block.rs` and TUI block creation paths — add canonical timestamp/hash fields to the shared block model.
- `crates/clankers-controller/src/convert.rs`, `src/modes/session_restore.rs`, and attach/history replay paths — derive the same block metadata during restore and daemon replay.
- `crates/clankers-message/` or a small shared helper module — define the canonical block-hashing input and BLAKE3 helper.
- TUI rendering and block-oriented UX — render original timestamps consistently after replay and make stable block identity available for future block-focused features.
- `Cargo.toml` / affected crate manifests — ensure `blake3` is available where canonical block hashes are computed.
