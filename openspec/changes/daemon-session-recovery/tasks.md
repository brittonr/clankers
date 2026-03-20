## 1. Session Catalog (redb)

- [x] 1.1 Define `SessionCatalogEntry` and `SessionLifecycle` types in `session_store.rs`
- [x] 1.2 Add `SESSION_CATALOG` and `SESSION_KEYS` redb table definitions alongside existing auth tables
- [x] 1.3 Implement catalog CRUD: `insert_session`, `update_session`, `get_session`, `list_sessions`, `remove_session`
- [x] 1.4 Implement key index CRUD: `insert_key`, `lookup_key`, `remove_keys_for_session`, `list_keys`
- [x] 1.5 Wire catalog writes into `spawn_agent_process` (insert on create) and `drain_and_broadcast` (update `last_active`/`turn_count` on a 5s timer)
- [x] 1.6 Wire catalog state transitions: `tombstoned` on `KillSession`, `tombstoned` on idle reaper
- [x] 1.7 Add catalog GC task: periodic sweep of tombstoned entries older than retention period (default 7 days)
- [ ] 1.8 Tests for catalog CRUD and lifecycle transitions

## 2. Checkpoint on Shutdown

- [x] 2.1 Extend `SessionController::shutdown()` to flush pending messages to `SessionManager`
- [x] 2.2 After `process_registry.shutdown_all()` in `run_daemon`, iterate `DaemonState` and transition all `active` catalog entries to `suspended`
- [x] 2.3 On startup, treat any `active` catalog entries as `suspended` (previous daemon crashed)
- [ ] 2.4 Tests for checkpoint flush and state transitions on shutdown

## 3. Session Recovery on Startup

- [x] 3.1 Add `SessionHandle` variant for placeholder/suspended sessions (no `cmd_tx`/`event_tx`) — use `Option` wrappers or a `SessionHandleState` enum
- [x] 3.2 On daemon startup, read `suspended` entries from catalog and populate `DaemonState` with placeholder handles
- [x] 3.3 Update `session_summaries()` and `status()` to include placeholder sessions with their lifecycle state
- [x] 3.4 Update `SessionSummary` in `clankers-protocol` to include a `state` field (active/suspended/recovering)
- [x] 3.5 Implement `recover_session()`: open automerge file, extract messages, `spawn_agent_process`, seed messages, replace placeholder handle, transition catalog to `active`
- [x] 3.6 Add `SessionCommand::SeedMessages` to load recovered conversation into the controller
- [x] 3.7 Handle corrupt/missing automerge file: log warning, start fresh session
- [x] 3.8 Wire lazy recovery into control socket `AttachSession` handler — trigger `recover_session` for suspended sessions before attaching
- [x] 3.9 Wire lazy recovery into `get_or_create_keyed_session` — recover suspended sessions matched by key
- [ ] 3.10 Tests for recovery from suspended catalog entries, corrupt file handling, and placeholder-to-live transition

## 4. QUIC Reconnect

- [ ] 4.1 In `run_quic_attach_loop` (attach.rs), store the session ID from initial `AttachResponse::Ok`
- [ ] 4.2 Implement `try_quic_reconnect` that opens a new bi stream with `DaemonRequest::Attach { session_id }` on the existing connection
- [ ] 4.3 If the iroh `Connection` is dead, attempt `endpoint.connect()` to re-establish, then retry attach
- [ ] 4.4 Wire reconnect into the disconnect detection path in the QUIC event loop (mirror Unix socket backoff: 5 attempts, 1s/2s/4s/8s/16s)
- [ ] 4.5 In `handle_attach_stream` (daemon side), trigger lazy recovery for suspended sessions before attaching
- [ ] 4.6 Tests for QUIC reconnect with session ID preservation

## 5. Graceful Restart

- [ ] 5.1 Add `ControlCommand::RestartDaemon` and `ControlResponse::Restarting` to `clankers-protocol`
- [ ] 5.2 Add `clankers daemon restart` CLI subcommand that sends `RestartDaemon` to control socket
- [ ] 5.3 Handle `RestartDaemon` in daemon: run checkpoint sequence, exit with code 75
- [ ] 5.4 In CLI daemon-start wrapper, detect exit code 75 and re-launch the daemon
- [ ] 5.5 Add `drain_timeout_secs` to `DaemonConfig` (default: 10)
- [ ] 5.6 Tests for restart flow: checkpoint + exit code + re-launch

## 6. Integration and Polish

- [ ] 6.1 Update `clankers ps` output to show session state column (active/suspended/recovering)
- [ ] 6.2 Update `clankers daemon status` to include recovery stats (sessions recovered, pending, failed)
- [ ] 6.3 Verify Matrix bridge works with recovered sessions (message arrives → lazy recovery → response)
- [ ] 6.4 End-to-end manual test: create sessions, stop daemon, restart, attach, verify history
