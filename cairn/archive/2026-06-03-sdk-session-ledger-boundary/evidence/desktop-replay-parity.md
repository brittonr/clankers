Task-ID: V2
Covers: sdk-session-ledger-boundary.verification.desktop-parity
Artifact-Type: validation-evidence

# Desktop Restore / Attach Replay Parity

## Added/covered tests

- Added `crates/clankers-controller/src/convert.rs::desktop_history_replay_parity_contract_covers_tool_compaction_branch_and_semantics` to cover attach/history replay projection for timestamps, tool results, compaction events, branch-summary app-edge metadata, and semantic tool event conversion.
- Existing `src/modes/session_restore.rs::restore_display_blocks_preserves_started_at_and_finalized_hash` covers standalone restore timestamps, tool-call input, response count, and finalized hashes.
- Existing `src/modes/session_restore.rs::restore_display_blocks_does_not_stamp_wall_clock_rebuild_time` covers deterministic restore timestamps/hashes rather than ambient rebuild time.

## Commands

- `nix develop -c cargo nextest run -p clankers-controller desktop_history_replay_parity_contract`
- `nix develop -c cargo nextest run -p clankers restore_display_blocks`

Both focused runs passed.
