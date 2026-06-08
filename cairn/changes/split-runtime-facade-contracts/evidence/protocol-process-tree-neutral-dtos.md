Artifact-Type: validation-log
Task-ID: I7,V6
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved daemon control-plane process-tree DTOs to neutral message contracts:

- Added `clanker_message::ProcessInfo` and `clanker_message::ProcessState` for actor/process-tree inspection surfaces.
- Re-exported both from `clankers-protocol::types` so existing `ControlResponse::Tree` wire shape and public protocol paths remain stable.
- Kept `clankers-protocol` responsible for control response framing/serialization; only reusable DTO ownership moved to the neutral contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clanker-message process_info_roundtrip_preserves_tree_fields --lib
cargo test -p clanker-message process_state_roundtrip --lib
cargo test -p clankers-protocol --lib
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
