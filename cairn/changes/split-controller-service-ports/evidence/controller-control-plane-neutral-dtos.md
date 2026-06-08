Artifact-Type: validation-log
Task-ID: I10,V9
Covers: r[remaining-coupling-drain.controller-service-ports.projection-owners], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Moved controller-facing daemon control-plane DTO ownership to neutral message contracts while keeping wire construction at the protocol adapter seam.

- `clanker_message::SessionSummary` and `clanker_message::DaemonStatus` now own the session-list/status data shapes.
- `clankers-protocol` re-exports those neutral DTOs for stable control response wire APIs.
- `clankers-controller::transport` and `transport_convert` now import session/status/process/plugin DTOs from `clanker_message`, leaving protocol-specific constructors (`ControlResponse`, `AttachResponse`, `DaemonEvent::SessionInfo`, `Handshake`) in `transport_convert`.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clankers --no-run
cargo test -p clanker-message session_summary_defaults_missing_state_for_legacy_wire_events --lib
cargo test -p clanker-message daemon_status_roundtrip_preserves_counters --lib
cargo test -p clankers-protocol --lib
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
