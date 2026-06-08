Artifact-Type: validation-log
Task-ID: I6,V5
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved additional protocol payload DTOs to neutral message contracts:

- Added `clanker_message::ImageData` for base64 image payloads used by prompts and tool output projection edges.
- Added `clanker_message::SerializedMessage` for seed/replay message payloads.
- Re-exported both from `clankers-protocol::types` so existing wire protocol paths and serialized JSON remain stable.
- Left protocol command/event/frame ownership in `clankers-protocol`; only reusable payload structs moved to the neutral contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clanker-message image_data_roundtrip --lib
cargo test -p clanker-message serialized_message_roundtrip_preserves_optional_fields --lib
cargo test -p clankers-protocol --lib
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0. An earlier attempted `cargo test -p clanker-message --lib image_data` filter matched 0 tests before the focused message-contract tests were added and was not used as evidence.
