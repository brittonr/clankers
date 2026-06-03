Evidence-ID: daemon-actor-loop-service-drain-tick-service-drain
Task-ID: V1,V2
Artifact-Type: command-log
Covers: daemon-actor-loop-service-drain.loop-inputs, daemon-actor-loop-service-drain.service-owner, daemon-actor-loop-service-drain.socketless-fixtures, daemon-actor-loop-service-drain.verification
Status: complete

# Daemon Actor Loop Tick Service Drain Evidence

## Implementation summary

- Added `DaemonSessionTickService` in `src/modes/daemon/session_plugins.rs` to own daemon session background projections: tool inventory refresh, controller event drains, and asynchronous plugin runtime UI drains.
- `DaemonSessionRuntime` now assembles `actor_tick_service` before `run_agent_actor`, and `agent_process.rs` calls service methods instead of inline `sync_tool_inventory`, `drain_and_broadcast`, or plugin runtime drain policy.
- Added socketless tick-service fixtures for inventory refresh and post-command tool-list projection.
- Kept actor-loop call path covered by the live stdio plugin refresh actor fixture and the plugin UI/display bridge fixture.
- Updated FCIS and lego architecture rails so `DaemonSessionTickService` is the named owner and selected drain policy cannot return to `agent_process.rs`.

## Focused daemon service and actor-path tests

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers tick_service
```

Result: 3 tests run, 3 passed, 1535 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers shared_plugin_disconnect_and_reconnect_updates_all_sessions
```

Result: 1 test run, 1 passed, 1537 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers daemon_bridge_forwards_stdio_plugin_ui_and_display_events
```

Result: 1 test run, 1 passed, 1537 skipped.

## Build and architecture rails

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers --tests
```

Result: exit status 0.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller --test fcis_shell_boundaries
```

Result: 44 tests run, 44 passed, 0 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= ./scripts/check-lego-architecture-boundaries.rs
```

Result: exit status 0; inventory written to `target/lego-architecture/dependency-ownership-inventory.json`.

```text
nix run .#cairn -- gate proposal daemon-actor-loop-service-drain --root .
nix run .#cairn -- gate design daemon-actor-loop-service-drain --root .
nix run .#cairn -- gate tasks daemon-actor-loop-service-drain --root .
```

Result: all three gates returned `valid: true` and `verdict: PASS`.

```text
nix run .#cairn -- validate --root .
```

Result before archive: `valid: true`; 4 active changes and 55 specs validated.

```text
nix run .#cairn -- archive daemon-actor-loop-service-drain --root . --execute
nix run .#cairn -- validate --root .
```

Result after archive: archive returned `mutated: true`; validation returned `valid: true` with 3 active changes and 54 specs validated.

```text
git diff --check
```

Result: exit status 0.
