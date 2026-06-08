Artifact-Type: validation-log
Task-ID: I5,V4
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Removed a duplicate daemon-protocol `PluginSummary` struct by reusing the neutral plugin display contract that already lives in `clanker-message`.

- `clankers-protocol::event` now re-exports `clanker_message::PluginSummary` next to the neutral `ToolInfo` re-export.
- `DaemonEvent::PluginList` keeps the same serialized shape while plugin summary ownership stays in the shared contract crate.
- The protocol crate remains responsible for frame/event serialization only; reusable plugin display DTOs stay outside the daemon protocol crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers --tests
cargo test -p clankers-protocol --lib
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
```

All commands exited 0.
