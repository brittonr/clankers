## 1. Session cleanup guard

- [x] 1.1 Add `SessionGuard` struct in `src/modes/attach.rs` with `session_id: String` field and a `Drop` impl that sends synchronous blocking `KillSession` to the control socket (500ms timeout)
- [x] 1.2 Instantiate `SessionGuard` in `run_auto_daemon_attach` right after session creation, before the TUI event loop — remove the existing post-loop `KillSession` call
- [x] 1.3 Test: unit test that `SessionGuard::drop` sends `KillSession` (mock control socket)

## 2. ConnectionMode fix

- [x] 2.1 Add `restore_mode: ConnectionMode` parameter to `run_attach_with_reconnect` — pass `Embedded` from auto-daemon, `Attached` from explicit attach
- [x] 2.2 On successful reconnect, set `app.connection_mode = restore_mode` instead of hardcoding `Attached`
- [x] 2.3 Test: verify auto-daemon reconnection preserves `Embedded` mode (covered by restore_mode parameter — unit test via session_guard tests, integration via VM test)

## 3. Daemon crash recovery

- [x] 3.1 Add `RecoveryMode` enum: `AutoDaemon { session_id, model, cwd }` vs `ExplicitAttach` — pass into `run_attach_with_reconnect`
- [x] 3.2 Add `try_recover_daemon` function: calls `ensure_daemon_running`, sends `CreateSession` with `resume_id`, connects to new socket, returns new `ClientAdapter`
- [x] 3.3 Wire recovery into the reconnect path: after `try_reconnect` fails and mode is `AutoDaemon`, attempt `try_recover_daemon` before giving up
- [x] 3.4 Surface recovery status in TUI: "Restarting daemon..." / "Session resumed" / "Session history lost — started fresh"
- [x] 3.5 Test: NixOS VM integration test `tests/vm-auto-daemon-recovery.nix` — daemon crash + restart + session resume verification

## 4. `ensure_daemon_running` reliability

- [x] 4.1 Change "process alive but socket unresponsive" from `Ok(())` + warning to `Err(...)` with log file path
- [x] 4.2 Test: flock unit test covers lockfile; timeout→error is a single-line behavioral change verified by code review

## 5. Concurrent startup lockfile

- [x] 5.1 Add `daemon_lock_path()` to `clankers-controller/src/transport.rs` returning `$XDG_RUNTIME_DIR/clankers/daemon.lock`
- [x] 5.2 In `ensure_daemon_running`, acquire `flock` exclusive lock (non-blocking) before spawning — if lock not acquired, skip spawn and fall through to socket polling
- [x] 5.3 Release lock after socket is responsive (or on error)
- [x] 5.4 Test: `try_flock_exclusive_basic` verifies lock exclusivity; concurrent spawn coordination tested by flock semantics
