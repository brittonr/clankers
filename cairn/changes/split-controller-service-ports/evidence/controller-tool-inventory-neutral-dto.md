Artifact-Type: validation-log
Task-ID: I9,V8
Covers: r[remaining-coupling-drain.controller-service-ports.projection-owners], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Moved controller tool-inventory metadata off a protocol-owned DTO and into the neutral message contract layer.

- Added `clanker_message::ToolInfo` next to other serde-friendly LLM/tool contracts.
- Re-exported `ToolInfo` from `clankers-protocol` so existing wire `DaemonEvent::ToolList` JSON remains stable while the concrete struct owner is neutral.
- Changed `SessionController::current_tool_infos()` and refresh comparison logic to construct `clanker_message::ToolInfo` instead of `clankers_protocol::ToolInfo`.
- Left daemon/protocol event projection at the declared `DaemonEvent::ToolList` edge; protocol remains responsible for frame/event serialization, not tool-inventory policy.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clanker-message tool_info_defaults_missing_source_for_legacy_wire_events --lib
cargo test -p clankers-protocol test_round_trip_remaining_daemon_events --lib
cargo test -p clankers-protocol --lib
cargo test -p clankers-controller refresh_tools --lib
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0. An earlier attempted protocol filter `tool_list` matched 0 tests and was not used as evidence.
