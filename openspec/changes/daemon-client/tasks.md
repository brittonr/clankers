# daemon-client — Tasks

## Phase 0: Protocol Crate

- [x] Create `crates/clankers-protocol/` with `Cargo.toml` (deps: serde, serde_json only)
- [x] Define `SessionCommand` enum with all variants
- [x] Define `DaemonEvent` enum with all variants
- [x] Define `ImageData`, `Handshake`, `SessionSummary`, `ProcessInfo` structs
- [x] Define `ControlCommand` / `ControlResponse` enums
- [x] Move `write_frame` / `read_frame` from `src/modes/rpc/iroh/protocol.rs` to `clankers-protocol` (generic over `AsyncWrite`/`AsyncRead`)
- [x] Add serde round-trip tests for every enum variant
- [x] Define `ToolBlocked` and `Capabilities` event variants
- [x] Add `GetCapabilities` command variant
- [x] Add `token: Option<String>` to `ControlCommand::CreateSession`
- [x] Write `daemon_event_from_tui_event()` and `tui_event_from_daemon_event()` converters
- [x] Verify: `cargo nextest run -p clankers-protocol`

## Phase 1: SessionController Extraction

- [x] Create `crates/clankers-controller/` with `Cargo.toml`
- [x] Define `SessionController` struct with moved fields (agent, session_mgr, loop_engine, hooks, audit, db)
- [x] Define `ControllerConfig` struct (settings, model, system_prompt, capabilities)
- [x] Move `drain_agent_events()` logic from `EventLoopRunner` to `SessionController::drain_events()`
- [x] Move event_translator.rs conversion into controller's drain path
- [x] Move session persistence (JSONL write on TurnEnd/UserInput) to controller
- [x] Move `AuditTracker` to controller
- [x] Move loop engine integration (`maybe_continue_loop`, `ensure_loop_registered`) to controller
- [x] Move auto-test trigger logic to controller
- [x] Implement capability enforcement in controller: filter tools at creation, check at execution
- [x] Implement capability delegation for child agents (child ⊆ parent clamping)
- [x] Move bash confirmation routing: controller holds oneshot senders, assigns request_ids, emits `ConfirmRequest`
- [x] Move todo request routing: same pattern as bash confirms
- [x] Move agent command dispatch (agent_task.rs logic) into `SessionController::handle_command()`
- [x] Wire `EventLoopRunner` to use `SessionController` internally (embedded mode)
- [x] Verify: all existing tests pass, TUI behavior unchanged
- [x] Write unit tests for `SessionController` with mock provider (no terminal needed)

## Phase 2: Unix Socket Transport

- [x] Add socket directory creation in `clankers daemon` startup
- [x] Implement PID file creation and stale detection
- [x] Implement control socket listener (accept `ControlCommand`, respond with `ControlResponse`)
- [x] Implement `ControlCommand::ListSessions` handler
- [x] Implement `ControlCommand::CreateSession` — spawns `SessionController`, creates session socket
- [x] Implement `ControlCommand::AttachSession` — returns socket path for existing session
- [x] Implement `ControlCommand::ProcessTree` handler
- [x] Implement `ControlCommand::KillSession` handler
- [x] Implement `ControlCommand::Shutdown` handler
- [x] Implement session socket listener — accepts connections, runs handshake, adds client to broadcast list
- [x] Implement session socket writer — serializes `DaemonEvent`, broadcasts to all connected clients
- [x] Implement session socket reader — deserializes `SessionCommand`, feeds to controller
- [x] Implement multi-client broadcast (each client gets all events)
- [x] Implement prompt serialization (reject concurrent prompts from different clients)
- [x] Handle client disconnect (remove from broadcast, agent continues)
- [x] Socket cleanup on graceful shutdown
- [x] Stale socket cleanup on startup
- [x] Verify: `clankers daemon` starts, `socat` can connect and see JSON frames

## Phase 3: TUI Client Mode

- [ ] Add `--daemon` flag to CLI (connect to daemon instead of in-process)
- [ ] Add `clankers attach [session-id]` subcommand
- [x] Implement `ClientAdapter<S: AsyncRead + AsyncWrite>` — generic over transport stream
- [x] Implement `ClientAdapter` event reader — stream → `DaemonEvent` → `App::handle_tui_event()`
- [x] Implement `ClientAdapter` command writer — `SessionCommand` → stream
- [ ] Instantiate `ClientAdapter` with `UnixStream` for local connections
- [ ] Implement history replay on attach (`ReplayHistory` → `HistoryBlock`* → `HistoryEnd`)
- [x] Split slash command registry into agent-side and client-side categories
- [x] Client-side slash commands handled locally (zoom, layout, panel, theme, copy, help, quit)
- [x] Agent-side slash commands sent as `SessionCommand::SlashCommand`
- [x] Implement bash confirmation UI over protocol (`ConfirmRequest` → render prompt → `ConfirmBash`)
- [ ] Implement todo request UI over protocol
- [x] Implement subagent event routing from `DaemonEvent` to SubagentPanel/SubagentPaneManager
- [ ] Add daemon indicator to status bar
- [ ] Implement `/detach` command (disconnect without killing agent)
- [ ] Implement reconnection with exponential backoff
- [ ] Verify: TUI in client mode looks and feels identical to embedded mode

## Phase 4: Actor Layer

- [x] Create `crates/clankers-actor/` with `Cargo.toml` (deps: tokio, dashmap, serde)
- [x] Implement `ProcessId` type (u64, monotonic, never reused)
- [x] Implement `Signal` enum (Message, Kill, Shutdown, Link, UnLink, LinkDied, Monitor, StopMonitoring, ProcessDied)
- [x] Implement `DeathReason` enum
- [x] Implement `ProcessHandle` struct
- [x] Implement `ProcessRegistry` with DashMap
- [x] Implement `registry.spawn()` — wraps tokio::spawn with signal channel and death notification loop
- [x] Implement linking: bidirectional death notification between linked processes
- [ ] Implement `die_when_link_dies` flag (default true)
- [x] Implement hierarchical shutdown: parent Kill → children Shutdown → timeout → children Kill
- [x] Implement monitoring (unidirectional, monitor gets `ProcessDied`)
- [x] Implement `SupervisorStrategy` enum (OneForOne, OneForAll, RestForOne)
- [x] Implement `Supervisor` — watches children, restarts per strategy, tracks restart rate
- [x] Implement max restart rate enforcement (shut down supervisor on too many restarts)
- [ ] Wrap `SessionController` in `AgentProcess` actor
- [ ] Root daemon becomes a `Supervisor` that spawns `AgentProcess` actors
- [ ] Migrate `SubagentTool` to spawn child `AgentProcess` via registry instead of `clankers -p`
- [ ] Migrate `DelegateTool` to spawn child `AgentProcess` for local workers
- [ ] Implement capability delegation enforcement (child ⊆ parent)
- [ ] Event forwarding: child `DaemonEvent` → parent `SubagentOutput` / `SubagentDone` / `SubagentError`
- [ ] Verify: subagent/delegate tools work through actor system, process tree visible

## Phase 5: Polish

- [ ] Implement `clankers ps` command (query `ProcessTree` via control socket, render tree)
- [ ] Implement `clankers kill <session-id>` command
- [ ] Implement `clankers attach` with no args (attach to most recent session)
- [ ] Add `--auto-daemon` flag (auto-start daemon if not running, then attach)
- [ ] Add `--read-only` flag (creates session with read-only capability token)
- [ ] Add `--capabilities` flag (comma-separated capability spec for session scoping)
- [ ] Graceful shutdown cascade: SIGTERM → Shutdown to all agents → wait → cleanup
- [ ] Session idle reaping in daemon (configurable timeout)
- [ ] Implement `IrohBiStream` wrapper (combines `SendStream` + `RecvStream` into `AsyncRead + AsyncWrite`)
- [ ] Define `clankers/session/1` ALPN constant
- [ ] Add `clankers/session/1` handler to daemon's iroh endpoint (same protocol as Unix socket)
- [ ] Add `clankers attach --remote <node-id>` subcommand
- [ ] Implement remote attach: create local iroh endpoint, connect to node, open bidi stream, wrap in `IrohBiStream`, instantiate `ClientAdapter`
- [ ] Add UCAN token requirement for remote connections (reject if no token in handshake)
- [ ] Support `--remote <peer-name>` lookup from `peers.json` in addition to raw node IDs
- [ ] Add remote status bar indicator (`🌐 node-id-short`)
- [ ] Add process resource tracking (memory, CPU, uptime) to `ProcessInfo`
- [ ] Documentation: update AGENTS.md, README, man page

## Phase 5b: CRDT Session Layer

- [ ] Add `automerge` dependency to `clankers-session` Cargo.toml
- [ ] Define Automerge document schema for sessions (header, messages map, roots list, annotations map)
- [ ] Implement `SessionManager` backed by `automerge::AutoCommit` instead of JSONL file
- [ ] Implement `append_message()` as Automerge map insert (keyed by MessageId)
- [ ] Implement `load_tree()` from Automerge document state
- [ ] Implement `save()` — serialize Automerge doc to `.automerge` binary file
- [ ] Implement `load()` — deserialize `.automerge` binary file
- [ ] Remove `BranchEntry` from `SessionEntry` — branching is implicit in parent-pointer DAG
- [ ] Remove `merge_branch()` — replace with Automerge native merge (both branches visible in tree)
- [ ] Remove `merge_selective()` — replace with Automerge merge + view filter
- [ ] Simplify `cherry_pick()` — still creates new messages, but just regular Automerge writes
- [ ] Implement `MergeMarker` annotation for recording merge points
- [ ] Implement `clankers session migrate <id>` — convert JSONL to Automerge document
- [ ] Preserve JSONL as export format (`clankers session export` outputs JSONL)
- [ ] Implement compaction within Automerge (CompactionEntry annotation + exclude from context)
- [ ] Implement Automerge doc compaction (`doc.save()` to squash change history for file size)
- [ ] Verify: all session tests pass with Automerge backend
- [ ] Verify: branch/merge/cherry-pick workflows produce identical results

## Phase 5c: CRDT Shared State

- [ ] Implement todo list as Automerge document (items list with id, text, status, note, timestamps)
- [ ] Implement `TodoTool` writes directly to Automerge doc instead of oneshot channel
- [ ] Implement TUI todo panel reads from local Automerge replica
- [ ] Remove `todo_tx` / `todo_rx` channel pair from EventLoopRunner
- [ ] Implement napkin as Automerge text document
- [ ] Implement napkin import from existing `.agent/napkin.md` on first Automerge session
- [ ] Implement napkin export to markdown on save (for non-Automerge readers)
- [ ] Implement peer registry as Automerge document (peers map keyed by NodeId)
- [ ] Implement iroh-docs sync for session documents between daemon and clients
- [ ] Implement iroh-docs sync for todo documents
- [ ] Implement iroh-docs sync for peer registry
- [ ] Verify: concurrent writes from two TUI clients merge correctly
- [ ] Verify: offline-then-reconnect scenario merges without data loss

## Phase 6: Aspen Backend (optional)

- [ ] Define `SessionBackend` trait (append_entry, load_entries, list_sessions, save_document, load_document, store_blob, get_blob)
- [ ] Implement `LocalSessionBackend` (wraps current Automerge file persistence)
- [ ] Implement `AspenSessionBackend` using `aspen-client` crate
- [ ] Implement KV storage for session documents: `sessions/{id}/doc`, `sessions/{id}/meta`
- [ ] Implement KV scan for session listing: `ScanKeys { prefix: "sessions/" }`
- [ ] Implement blob storage for tool artifacts via `aspen-client::blob_client`
- [ ] Implement session entry references to blobs (store hash, not inline content) for large outputs
- [ ] Add `--backend aspen --ticket <ticket>` CLI flags
- [ ] Add `"backend"` and `"aspen_ticket"` to Settings
- [ ] Implement auth mapping: aspen capabilities → clankers capabilities
- [ ] Implement `AgentJobWorker` — wraps SessionController as an aspen `Worker`
- [ ] Implement subagent job submission: SubagentTool submits to aspen job queue when backend is aspen
- [ ] Implement iroh-docs sync via aspen's DocsExporter (reuse aspen's existing pipeline)
- [ ] Implement fallback: aspen unreachable → degrade to local backend with warning
- [ ] Verify: session persistence round-trips through aspen KV
- [ ] Verify: blob storage/retrieval for tool outputs
- [ ] Verify: subagent job execution across two aspen nodes
