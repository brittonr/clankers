Artifact-Type: validation-log
Task-ID: I8,V7
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved daemon control-plane session/status DTOs to neutral message contracts:

- Added `clanker_message::SessionSummary` and `clanker_message::DaemonStatus` for daemon session-list and status surfaces.
- Re-exported both from `clankers-protocol::control` and crate root so existing `ControlResponse::Sessions` / `ControlResponse::Status` JSON and public protocol paths remain stable.
- Kept `clankers-protocol` responsible for control command/response framing; only reusable DTO ownership moved to the neutral contract crate.

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
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
