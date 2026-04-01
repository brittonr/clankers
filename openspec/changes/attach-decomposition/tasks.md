## 1. Clippy fixes

- [x] 1.1 Fix collapsible `if` warnings in `tests/nix_integration.rs` (2 instances)
- [x] 1.2 Fix `unnecessary_join` warning in `tests/schedule_integration.rs`
- [x] 1.3 Fix clippy warnings in `tests/socket_bridge.rs`
- [x] 1.4 Run `cargo clippy --all-targets` and confirm zero warnings

## 2. Extract remote attach module

- [x] 2.1 Create `src/modes/attach_remote.rs` with QUIC types and functions moved from `attach.rs`
- [x] 2.2 Add necessary imports and adjust visibility (`pub(crate)` for internal items)
- [x] 2.3 Add `mod attach_remote;` to `src/modes/mod.rs`
- [x] 2.4 Add `pub use attach_remote::*;` re-export in `attach.rs`
- [x] 2.5 Run `cargo check` — confirm no compilation errors

## 3. Extract auto-daemon module

- [x] 3.1 Create `src/modes/auto_daemon.rs` with `AutoDaemonOptions`, `run_auto_daemon_attach`, `SessionGuard`, and helpers moved from `attach.rs`
- [x] 3.2 Add necessary imports and adjust visibility
- [x] 3.3 Add `mod auto_daemon;` to `src/modes/mod.rs`
- [x] 3.4 Add `pub use auto_daemon::*;` re-export in `attach.rs`
- [x] 3.5 Run `cargo check` — confirm no compilation errors

## 4. Verify

- [x] 4.1 Run `cargo clippy --all-targets` — zero warnings
- [x] 4.2 Run `cargo nextest run` — all tests pass
- [x] 4.3 Confirm no import path changes outside `src/modes/`
