## Why

Auto-daemon mode (`use_daemon=true`, the default) routes interactive sessions through the daemon transparently. The wiring works, but several edge cases leak sessions, break UX on daemon crash, or expose implementation details the user shouldn't see.

## What Changes

- **Session cleanup on signal/panic**: Register a cleanup guard so `KillSession` fires even when the process exits via Ctrl+C or panic, preventing orphaned sessions from accumulating in the daemon.
- **Reconnection badge leak**: Auto-daemon reconnection currently flips `ConnectionMode` to `Attached`, showing a badge the user shouldn't see. Keep it `Embedded` throughout.
- **Daemon crash recovery**: When the daemon dies during an auto-daemon session, restart it, create a new session (resuming from automerge checkpoint), and reconnect — instead of telling the user to `/quit`.
- **`ensure_daemon_running` reliability**: Return an error instead of `Ok(())` when the socket never becomes responsive. Prevents confusing downstream failures.
- **Concurrent start race**: Use a lockfile around daemon startup so two simultaneous `clankers` invocations don't both spawn daemon processes.

## Capabilities

### New Capabilities
- `auto-daemon-lifecycle`: Session cleanup guarantees, daemon crash recovery, and startup coordination for auto-daemon mode.

### Modified Capabilities

## Impact

- `src/modes/attach.rs` — `run_auto_daemon_attach`, `run_attach_with_reconnect`, `try_reconnect`
- `src/commands/daemon.rs` — `ensure_daemon_running`
- `crates/clankers-controller/src/transport.rs` — lockfile helpers if needed
- Tests: NixOS VM integration test covering crash-and-recover, plus unit tests for cleanup guard and lockfile
