## Why

`src/modes/attach.rs` is 2548 lines handling three distinct concerns: local Unix socket attach, remote QUIC attach, and auto-daemon lifecycle. The QUIC code (~500 lines) shares no state with local attach beyond the final event loop. Clippy warnings in test files (`nix_integration.rs`, `schedule_integration.rs`, `socket_bridge.rs`) have accumulated and should be cleaned up alongside.

## What Changes

- Extract remote/QUIC attach logic into `src/modes/attach_remote.rs` (~500 lines: `QuicBiStream`, `run_remote_attach`, `run_remote_attach_loop`, QUIC framing helpers, reconnection)
- Extract auto-daemon lifecycle into `src/modes/auto_daemon.rs` (~200 lines: `AutoDaemonOptions`, `run_auto_daemon_attach`, `SessionGuard`, `ensure_daemon_running`)
- Fix all current clippy warnings in test files (collapsible `if`, `unnecessary_join`)
- `attach.rs` retains local attach, event processing, key handling, slash commands (~1800 lines)
- No public API changes — re-exports from `attach.rs` preserve all call sites

## Capabilities

### New Capabilities
- `attach-module-split`: Decomposition of attach.rs into three modules with re-exports

### Modified Capabilities

## Impact

- `src/modes/attach.rs` → split into `attach.rs` + `attach_remote.rs` + `auto_daemon.rs`
- `src/modes/mod.rs` — add new module declarations
- `tests/nix_integration.rs` — fix collapsible `if` warnings
- `tests/schedule_integration.rs` — fix `unnecessary_join` warning
- `tests/socket_bridge.rs` — fix warnings
- No dependency or API changes
