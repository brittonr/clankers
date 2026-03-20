## Context

The daemon runs agent sessions as actors in a `ProcessRegistry`. All state lives in memory: `DaemonState` holds `SessionHandle` entries with `cmd_tx`/`event_tx` channels. Sessions persist conversation history to automerge files via `SessionManager`, but the daemon never reads them back. On restart, the daemon starts empty.

Clients can reconnect to a *running* daemon over Unix sockets (with exponential backoff). Remote QUIC clients detect disconnection but have no working reconnect path — the napkin notes the socket path is empty for remote connections.

The auth layer already uses redb (`clankers.db`) for token storage. The same database can host session catalog tables.

## Goals / Non-Goals

**Goals:**
- Daemon restart is a non-event for users: sessions survive and clients reconnect
- Lazy recovery: don't spawn 30 actors on startup if nobody's connecting
- Remote QUIC clients can reconnect to the same session after network failures or daemon restart
- `clankers daemon restart` for upgrades without losing conversation state

**Non-Goals:**
- Live migration of in-flight LLM streaming (turns in progress are lost on restart)
- Session transfer between different daemon instances / machines
- Backward compat with pre-catalog daemons (new sessions only; old sessions use `--resume`)
- High-availability / replicated daemon

## Decisions

### 1. Session catalog in redb (same DB as auth)

Add two tables to the existing `clankers.db`:

```
SESSION_CATALOG: session_id → SessionCatalogEntry (bincode)
SESSION_KEYS:    SessionKey (json) → session_id
```

`SessionCatalogEntry`:
```rust
struct SessionCatalogEntry {
    session_id: String,
    automerge_path: PathBuf,
    model: String,
    created_at: String,       // ISO 8601
    last_active: String,      // ISO 8601
    turn_count: usize,
    state: SessionLifecycle,  // Active, Suspended, Tombstoned
}

enum SessionLifecycle { Active, Suspended, Tombstoned }
```

**Why redb over SQLite**: Already a dependency for auth. Single-file, no extra process. Catalog is simple key-value — no queries that benefit from SQL.

**Why not automerge for the catalog**: The catalog is daemon-private metadata, not collaborative data. Write-heavy (every prompt updates `last_active`). redb is purpose-built for this.

**Alternative: filesystem scan on startup**. Rejected — scanning `~/.clankers/sessions/` and parsing every automerge file is O(n) in total session count, not just active ones. The catalog is O(active sessions).

### 2. Lazy actor spawning with placeholder handles

On startup, `DaemonState` gets populated from the catalog but with *placeholder* `SessionHandle` entries that have no `cmd_tx`/`event_tx`. The placeholders carry enough metadata for `clankers ps` to display them.

When a client attaches or a message arrives for a placeholder:
1. Open the automerge file, extract messages
2. Call `spawn_agent_process()` with the recovered session ID
3. Seed the controller with the extracted messages via a new `SessionCommand::SeedMessages`
4. Replace the placeholder handle with a live one
5. Transition catalog entry to `active`

```
┌────────────┐     attach/     ┌────────────┐     spawn +    ┌────────────┐
│ Suspended  │────message────▶│ Recovering │────rehydrate──▶│  Active    │
│ (placeholder)│               │ (spawning) │                │ (live actor)│
└────────────┘                └────────────┘                └────────────┘
```

**Why lazy**: A daemon that managed 20 sessions yesterday shouldn't spawn 20 LLM sessions on restart. Most are probably idle. Only revive what's needed.

**Why `SeedMessages` instead of constructor arg**: `spawn_agent_process` already takes `model` and `system_prompt`. Adding a messages vec complicates the hot path. A post-spawn seed command is cleaner and lets the actor handle it in its normal event loop.

### 3. Checkpoint on shutdown via actor signal

The existing `Signal::Shutdown` already triggers `controller.shutdown()`. Extend `SessionController::shutdown()` to flush pending messages to the `SessionManager` before returning. The actor loop in `run_agent_actor` already calls `controller.shutdown()` on `Signal::Shutdown`.

After all actors exit (existing `process_registry.shutdown_all(5s)`), iterate `DaemonState` and flip every `active` catalog entry to `suspended`.

No new shutdown mechanism needed — the actor system already handles this. The gap is that `controller.shutdown()` doesn't explicitly flush persistence.

### 4. QUIC reconnect: session ID in handshake + new stream

The existing `Handshake` struct has `session_id: Option<String>`. The fix:

**Client side** (`run_quic_attach_loop`):
- Store the session ID from `AttachResponse::Ok`
- On stream loss, open a new bi stream on the same `Connection`
- Send `DaemonRequest::Attach` with the stored session ID
- If the `Connection` itself is dead, attempt `endpoint.connect()` again

**Daemon side** (`handle_attach_stream`):
- Already handles `session_id: Some(id)` — looks up the session
- For suspended sessions: trigger lazy recovery, then attach
- No protocol changes needed, just plumbing

**Why not a reconnect token**: The session ID is sufficient. Auth is handled by the UCAN token in the handshake. Adding a reconnect token adds complexity for no security gain — if you have the UCAN token, you can attach to any session you're authorized for.

### 5. Restart command: checkpoint + re-exec

`clankers daemon restart` sends `ControlCommand::RestartDaemon` to the control socket. The daemon:
1. Sends `Shutdown` to all actors (existing)
2. Waits for drain timeout
3. Flushes catalog
4. Writes a `restart.lock` file with the new binary path
5. Exits with a conventional exit code (e.g., 75)

The CLI wrapper detects exit code 75 and re-launches. Simpler than `exec()` — avoids fd inheritance issues and works with systemd.

**Alternative: in-process re-exec via `std::os::unix::process::exec()`**. Rejected — file descriptors, locks, and mmap'd redb state make this fragile. Clean process boundary is safer.

## Risks / Trade-offs

**[Stale catalog entries]** → If the daemon crashes (SIGKILL, OOM), catalog entries stay `active` forever. Mitigation: on startup, treat all `active` entries as `suspended` (the daemon that owned them is dead).

**[Rehydration cost]** → Large sessions (1000+ turns) take time to parse from automerge. Mitigation: lazy recovery means this only happens when someone actually connects. Could add a rehydration timeout (5s) and fall back to truncated context.

**[redb lock contention]** → Catalog writes on every prompt (updating `last_active`). Mitigation: batch updates on a timer (every 5s) instead of per-prompt. The `last_active` field is advisory, not critical.

**[Restart gap window]** → Between daemon exit and restart, no daemon is running. Remote clients see disconnection. Mitigation: reconnect retry with backoff handles this. Gap is typically < 2 seconds.

**[SeedMessages ordering]** → Messages must be seeded before any client prompt arrives. Mitigation: the placeholder handle has no `cmd_tx`, so no commands can be sent until recovery completes and the handle is replaced.

## Open Questions

- Should `tombstoned` sessions be recoverable via `--resume`, or is tombstoning a hard delete of the catalog entry? (Proposed: tombstone is catalog-only; automerge file always survives for `--resume`.)
- Should the drain timeout be per-session or global? (Proposed: global, since the registry's `shutdown_all` already uses a single timeout.)
- Should the catalog track the `SessionKey` index as a separate table or embed keys in `SessionCatalogEntry`? (Proposed: separate table for clean lookup semantics.)
