## ADDED Requirements

### Requirement: Remote attach module extraction
All QUIC/iroh remote attach functionality SHALL reside in `src/modes/attach_remote.rs`. This includes `QuicBiStream`, `run_remote_attach`, `run_remote_attach_loop`, `try_quic_reconnect`, `try_quic_attach_stream`, `build_quic_client_adapter`, `create_remote_session`, `quic_write_frame`, and `quic_read_frame`.

#### Scenario: Remote attach compiles from extracted module
- **WHEN** `run_remote_attach` is called from `main.rs`
- **THEN** it resolves through `attach.rs` re-export to `attach_remote.rs` with no caller changes

#### Scenario: QUIC stream types are module-private
- **WHEN** `QuicBiStream` is defined in `attach_remote.rs`
- **THEN** it SHALL be `pub(crate)` or narrower, not `pub`

### Requirement: Auto-daemon module extraction
All auto-daemon lifecycle functionality SHALL reside in `src/modes/auto_daemon.rs`. This includes `AutoDaemonOptions`, `run_auto_daemon_attach`, `SessionGuard`, and `ensure_daemon_running`.

#### Scenario: Auto-daemon compiles from extracted module
- **WHEN** `run_auto_daemon_attach` is called from `main.rs`
- **THEN** it resolves through `attach.rs` re-export to `auto_daemon.rs` with no caller changes

#### Scenario: SessionGuard remains crate-private
- **WHEN** `SessionGuard` is defined in `auto_daemon.rs`
- **THEN** it SHALL NOT be `pub` — it is an internal implementation detail

### Requirement: Attach module re-exports
`src/modes/attach.rs` SHALL re-export all public items from `attach_remote.rs` and `auto_daemon.rs` so that existing callers do not need import path changes.

#### Scenario: No external import changes
- **WHEN** code outside `src/modes/` imports from `crate::modes::attach`
- **THEN** all previously available items SHALL still resolve

### Requirement: Clippy clean
All clippy warnings in test files SHALL be fixed: collapsible `if` in `tests/nix_integration.rs`, `unnecessary_join` in `tests/schedule_integration.rs`, and warnings in `tests/socket_bridge.rs`.

#### Scenario: Clippy passes with no warnings
- **WHEN** `cargo clippy --all-targets` is run
- **THEN** zero warnings SHALL be emitted
