## 1. Canonical block metadata model

- [x] I1 Add shared `started_at` and `finalized_hash` fields to `ConversationBlock`, introduce one pure canonical metadata builder/helper in `crates/clankers-tui-types`, thread it through every `ConversationBlock {` / constructor call site, and pin one versioned canonical BLAKE3 envelope fixture so renderers, replay logic, and machine-readable block consumers all read one shared metadata model. [covers=conversation.block.metadata.canonical.timestamp,conversation.block.metadata.finalized.blake3.hash,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields]
- [x] I2 Extend display-layer tool-call metadata and canonical-envelope field selection so hashed block content explicitly includes prompt text, assistant text, tool-call name/input, tool-result text/images/error flag, and canonical timestamp while excluding transient UI-only state such as local block IDs, collapse state, focus state, scroll state, streaming state, token counters, and timezone formatting. [covers=conversation.block.metadata.finalized.blake3.hash.content-changes-change-digest,conversation.block.metadata.finalized.blake3.hash.ui-state-does-not-change-digest,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields]

## 2. Live block construction and rendering

- [x] I3 Update live block creation/finalization plus block rendering to use the opening user-message timestamp, keep branched blocks distinct, defer final-hash publication while streaming, and finalize the BLAKE3 hash only after the block is complete. [covers=conversation.block.metadata.canonical.timestamp.live-block-opening-message-time,conversation.block.metadata.canonical.timestamp.branch-gets-distinct-time,conversation.block.metadata.shared.surfaces.streaming-blocks-defer-final-hash]

## 3. Restore and replay parity

- [x] I4 Update session restore and attach/controller history replay to preserve the original block timestamp, keep the replay block open across assistant/tool-result history until replay completes, and expose the same canonical metadata through shared `ConversationBlock` values in standalone and attach paths. [covers=conversation.block.metadata.canonical.timestamp.restore-preserves-original-time,conversation.block.metadata.finalized.blake3.hash.replay-of-same-sequence-matches,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields,conversation.block.metadata.shared.surfaces.standalone-and-attach-agree]

## 4. Verification

- [x] V1 Positive/negative: run `RUSTC_WRAPPER= cargo test -p clankers-tui-types --lib` and `RUSTC_WRAPPER= cargo test -p clankers-controller convert::tests:: --lib` to prove the pinned v1 envelope is stable, identical canonical content yields the same BLAKE3 hash, prompt/assistant/tool/timestamp changes change the hash, transient UI state does not affect the hash, and user-input timestamp propagation survives controller replay conversion. [covers=conversation.block.metadata.finalized.blake3.hash.content-changes-change-digest,conversation.block.metadata.finalized.blake3.hash.ui-state-does-not-change-digest,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields] [evidence=openspec/changes/conversation-block-metadata/evidence/canonical-hash-tests.txt]
- [x] V2 Positive: run `RUSTC_WRAPPER= cargo test -p clankers-tui --lib` to prove live `ConversationBlock` values expose canonical `started_at` / `finalized_hash` fields to machine-readable consumers, keep the opening timestamp stable for normal, tool-heavy, and branched turns, and keep finalized hashes pending until completion. [covers=conversation.block.metadata.canonical.timestamp.live-block-opening-message-time,conversation.block.metadata.canonical.timestamp.branch-gets-distinct-time,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields,conversation.block.metadata.shared.surfaces.streaming-blocks-defer-final-hash] [evidence=openspec/changes/conversation-block-metadata/evidence/live-block-tests.txt]
- [x] V3 Positive: run `CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= cargo test --lib modes::attach::tests::history_replay_matches_session_restore_block_metadata` and `CC=gcc CXX=g++ CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=gcc RUSTFLAGS='-C link-arg=-fuse-ld=bfd' RUSTC_WRAPPER= cargo test --lib modes::session_restore::tests::` to prove restored history no longer gets fresh timestamps after replay and the same persisted message sequence yields the same canonical timestamp and finalized hash in standalone restore and attach replay. [covers=conversation.block.metadata.canonical.timestamp.restore-preserves-original-time,conversation.block.metadata.finalized.blake3.hash.replay-of-same-sequence-matches,conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields,conversation.block.metadata.shared.surfaces.standalone-and-attach-agree] [evidence=openspec/changes/conversation-block-metadata/evidence/replay-parity-tests.txt]

## Verification Matrix

- `conversation.block.metadata.canonical.timestamp` -> `I1`, `I3`, `I4`, `V2`, `V3`
- `conversation.block.metadata.canonical.timestamp.live-block-opening-message-time` -> `I3`, `V2`
- `conversation.block.metadata.canonical.timestamp.restore-preserves-original-time` -> `I4`, `V3`
- `conversation.block.metadata.canonical.timestamp.branch-gets-distinct-time` -> `I3`, `V2`
- `conversation.block.metadata.finalized.blake3.hash` -> `I1`, `I2`, `I4`, `V1`, `V3`
- `conversation.block.metadata.finalized.blake3.hash.replay-of-same-sequence-matches` -> `I4`, `V3`
- `conversation.block.metadata.finalized.blake3.hash.content-changes-change-digest` -> `I2`, `V1`
- `conversation.block.metadata.finalized.blake3.hash.ui-state-does-not-change-digest` -> `I2`, `V1`
- `conversation.block.metadata.shared.surfaces` -> `I1`, `I3`, `I4`, `V1`, `V2`, `V3`
- `conversation.block.metadata.shared.surfaces.machine-readable-consumers-share-fields` -> `I1`, `I2`, `I4`, `V1`, `V2`, `V3`
- `conversation.block.metadata.shared.surfaces.standalone-and-attach-agree` -> `I4`, `V3`
- `conversation.block.metadata.shared.surfaces.streaming-blocks-defer-final-hash` -> `I3`, `V2`
