# Aspen Backend — Implementation Plan

## Architecture

A new crate **`crates/clankers-aspen/`** wraps `aspen-client` and provides
storage backends, sync services, and new agent tools that connect clankers to
a running Aspen cluster over iroh QUIC. The design is **additive** — Aspen is
always optional, local-first operation remains the default.

---

## Dependency Graph

```
Phase 0 (Foundation)
├── Phase 1 (Memory Sync)
├── Phase 2 (Secrets)
│   └── Phase 7 (Auth Bridge)
│       └── Phase 8 (Tools: Forge, CI, Jobs)
├── Phase 3 (Session Sync)
├── Phase 4 (Coordination / Distributed Locking)
├── Phase 5 (Usage Tracking Sync)
└── Phase 6 (Blob Sharing)
```

**Recommended execution order**: 0 → 1 → 2 → 5 → 4 → 6 → 3 → 7 → 8

This prioritizes: foundation first, then high-value/low-effort items (memory
sync, secrets, usage tracking), then the large session refactor, and finally
the tool suite.

---

## Phase 0: Foundation — Connection & Config

**Goal**: Clankers can connect to an Aspen cluster and verify health. No
behavioral changes yet.

**Depends on**: Nothing

**Complexity**: Medium (~2-3 days)

### New crate: `crates/clankers-aspen/`

```
crates/clankers-aspen/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Re-exports, feature gate
│   ├── connection.rs   # AspenConnection wrapper with lazy init + reconnect
│   ├── config.rs       # AspenConfig struct (ticket, token, timeouts)
│   ├── error.rs        # Snafu error types for Aspen operations
│   └── keys.rs         # Key namespace constants (prefixes for all KV data)
```

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/Cargo.toml` | Deps: `aspen-client`, `aspen-client-api`, `aspen-auth`, `tokio`, `serde`, `postcard`, `snafu`, `tracing` |
| `crates/clankers-aspen/src/lib.rs` | Public API surface, re-exports |
| `crates/clankers-aspen/src/config.rs` | `AspenConfig { ticket: String, token: Option<String>, rpc_timeout_secs: u64, enabled: bool }` |
| `crates/clankers-aspen/src/connection.rs` | `AspenConnection` — wraps `AspenClient`, lazy connect, health check, reconnect on failure |
| `crates/clankers-aspen/src/error.rs` | `AspenError` enum via snafu: `Connect`, `Rpc`, `Serialize`, `Auth`, `Timeout` |
| `crates/clankers-aspen/src/keys.rs` | Constants: `MEMORY_PREFIX = "clankers/memory/"`, `SESSION_PREFIX = "clankers/session/"`, `USAGE_PREFIX = "clankers/usage/"`, etc. |

### Files to modify

| File | Change |
|------|--------|
| `Cargo.toml` (workspace) | Add `clankers-aspen` to workspace members |
| `src/config/settings.rs` | Add optional `aspen: Option<AspenConfig>` field to `Settings` struct |
| `src/main.rs` | If `settings.aspen` is set, create `AspenConnection` and pass it through startup. Call `health_check()` on connect. |

### Aspen operations used

- `ClientRpcRequest::GetHealth` — verify cluster reachable
- `ClientRpcRequest::Ping` — connection keepalive

### Key design decisions

1. **`AspenConnection` wraps `AspenClient` with lifecycle management**: Lazy
   connection (don't block startup if cluster is unreachable), automatic
   reconnect with backoff, `Arc<AspenConnection>` passed through the app.

2. **Config lives in `settings.json`**:
   ```json
   {
     "aspen": {
       "ticket": "aspen1...",
       "token": "base64-encoded-ucan-token",
       "rpcTimeoutSecs": 5,
       "enabled": true
     }
   }
   ```
   Also support `CLANKERS_ASPEN_TICKET` and `CLANKERS_ASPEN_TOKEN` env vars
   as overrides.

3. **Feature-gated**: The `clankers-aspen` crate is behind a `aspen` feature
   flag on the main binary so builds without Aspen deps are possible.

### Testing strategy

- Unit test: `AspenConfig` serialization/deserialization
- Unit test: `AspenConnection` handles missing/invalid ticket gracefully
- Integration test: Connect to a test cluster, call `GetHealth`, verify response
- Test: Settings merge with and without `aspen` field

---

## Phase 1: Shared Memory

**Goal**: Agent memories sync bidirectionally between local redb and Aspen KV,
so all clankers instances on the cluster share knowledge.

**Depends on**: Phase 0

**Complexity**: Medium (~2-3 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/memory_sync.rs` | `MemorySyncService` — bidirectional sync between redb and Aspen KV |

### Files to modify

| File | Change |
|------|--------|
| `src/db/memory.rs` | Add `sync_to_aspen()` and `sync_from_aspen()` methods to `MemoryStore`. After `save()`, optionally push to Aspen. |
| `src/agent/mod.rs` | On agent startup (if Aspen configured), run initial memory sync from cluster before first turn. |
| `crates/clankers-aspen/src/lib.rs` | Re-export `MemorySyncService` |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `WriteKey { key: "clankers/memory/{id}", value }` | Push new memory to cluster |
| `ReadKey { key: "clankers/memory/{id}" }` | Read specific memory from cluster |
| `ScanKeys { prefix: "clankers/memory/" }` | List all shared memories |
| `DeleteKey { key: "clankers/memory/{id}" }` | Remove a memory from cluster |
| `CompareAndSwapKey` | Conflict-safe update when same ID is modified by two instances |

### Design

```
KV key format: clankers/memory/{id}
KV value: postcard-serialized MemoryEntry (same struct from db/memory.rs)
```

**Sync strategy — last-writer-wins with merge**:
1. On startup: `ScanKeys("clankers/memory/")` → merge into local redb (skip
   entries already present by ID, take newer `created_at` on conflict)
2. On `memory.save()`: write to redb first, then fire-and-forget `WriteKey` to
   Aspen
3. On `memory.remove()`: delete from redb first, then fire-and-forget `DeleteKey`
4. Periodic background sync (every 60s) to catch memories from other instances

**No locking needed** — memories are append-mostly and identified by unique
monotonic IDs. Conflicts (same ID, different content) are resolved by timestamp.

### Testing strategy

- Unit test: `MemoryEntry` round-trips through postcard serialization
- Unit test: Merge logic handles duplicate IDs correctly, newer wins
- Integration test: Two `MemoryStore` instances sync through Aspen, both see each other's memories
- Test: Offline mode — Aspen unreachable, local operations work fine, sync resumes when cluster returns

---

## Phase 2: Secrets Integration

**Goal**: API keys and credentials can be retrieved from Aspen's secrets
engine, replacing hardcoded env vars.

**Depends on**: Phase 0

**Complexity**: Small (~1-2 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/secrets.rs` | `SecretsProvider` — fetch API keys from Aspen secrets vault |

### Files to modify

| File | Change |
|------|--------|
| `src/config/settings.rs` | Add `secrets_mount: Option<String>` to `AspenConfig` (default: `"clankers"`) |
| `src/provider/anthropic/mod.rs` (and other providers) | Before falling back to `ANTHROPIC_API_KEY` env var, check `SecretsProvider` if configured |
| `crates/clankers-router/src/config.rs` | Router can also pull credentials from Aspen secrets |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `SecretsKvRead { mount, path, version }` | Read `clankers/anthropic_api_key`, `clankers/openai_api_key`, etc. |
| `SecretsKvList { mount, path }` | Discover available credential paths |
| `SecretsTransitDecrypt { key_name, ciphertext }` | Decrypt locally-cached encrypted credentials |

### Design

**Secret path convention**:
```
mount: "clankers"
paths:
  providers/anthropic/api_key
  providers/openai/api_key
  providers/google/api_key
  router/credential_pool/{n}
```

**Credential resolution order**:
1. Env var (e.g., `ANTHROPIC_API_KEY`) — explicit override always wins
2. Aspen secrets (`SecretsKvRead`)
3. Config file (existing behavior)

**Caching**: Secrets are cached in-memory with a TTL (default 5 minutes). The
`SecretsProvider` exposes `get_credential(provider_name) -> Option<String>`.

### Testing strategy

- Unit test: Credential resolution order is correct
- Unit test: Cache expiry works
- Integration test: Write a secret to Aspen, verify clankers reads it
- Test: Aspen unreachable falls back to env vars without error

---

## Phase 3: Session Sync

**Goal**: Sessions stored in Aspen blobs so you can resume a conversation
from any machine connected to the same cluster.

**Depends on**: Phase 0

**Complexity**: Large (~4-5 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/session_sync.rs` | `SessionSyncService` — upload/download session JSONL to Aspen blobs, maintain index in KV |
| `crates/clankers-aspen/src/session_index.rs` | KV-based session index for cross-machine discovery |

### Files to modify

| File | Change |
|------|--------|
| `src/session/store.rs` | Extract a `SessionStore` trait with `append_entry()`, `read_entries()`, `list_sessions()`. Current filesystem impl becomes `LocalSessionStore`. |
| `src/session/mod.rs` | `SessionManager` takes a `Box<dyn SessionStore>` or uses a composite `SyncedSessionStore` that writes locally + pushes to Aspen |
| `src/main.rs` | Wire up `SyncedSessionStore` when Aspen is configured |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `AddBlob { data, tag }` | Upload session JSONL file as a blob |
| `GetBlob { hash }` | Download session content |
| `GetBlobTicket { hash }` | Share session with another user |
| `WriteKey { key: "clankers/session_index/{cwd_hash}/{session_id}" }` | Index entry: blob hash, metadata, last message count |
| `ScanKeys { prefix: "clankers/session_index/{cwd_hash}/" }` | List sessions for a project |
| `ScanKeys { prefix: "clankers/session_index/" }` | List all sessions across projects |
| `CompareAndSwapKey` | Atomic index update when session grows |

### Design

**Two-tier storage**:
- **KV index**: Small metadata per session (blob hash, message count, last
  activity, model, cwd)
  ```
  Key:   clankers/session_index/{cwd_hash}/{session_id}
  Value: SessionIndexEntry { blob_hash, message_count, last_activity, model, cwd, created_at }
  ```
- **Blob store**: Actual JSONL content, content-addressed by BLAKE3 hash

**Sync protocol**:
1. **On append**: Write to local JSONL (fast path, always works). Then async:
   re-upload the full JSONL as a new blob, CAS-update the index entry with new
   blob hash and message count. Use the old blob hash as the CAS expected value
   to detect concurrent modifications.
2. **On session list**: Merge local filesystem sessions with Aspen KV index
   scan. Deduplicate by session_id.
3. **On session resume from another machine**: `GetBlob` to download the JSONL,
   write to local filesystem, open normally.
4. **Debounce uploads**: Don't upload after every single message. Buffer for 5
   seconds or until a turn completes, then upload once.

**`SessionStore` trait**:
```rust
#[async_trait]
pub trait SessionStore: Send + Sync {
    fn append_entry(&self, path: &Path, entry: &SessionEntry) -> Result<()>;
    fn read_entries(&self, path: &Path) -> Result<Vec<SessionEntry>>;
    fn list_sessions(&self, sessions_dir: &Path, cwd: &str) -> Vec<PathBuf>;
}
```

### Risks

- **Session file size**: Long sessions can be large (MBs). Aspen blobs handle
  this, but re-uploading the full JSONL on every append is wasteful. Mitigation:
  debounce + only upload on turn boundaries.
- **Concurrent modification**: Two clankers instances resume the same session
  simultaneously. Mitigation: CAS on the index entry detects this; the second
  writer gets a conflict error and can reload.

### Testing strategy

- Unit test: `SessionStore` trait implementations (local, synced) have same behavior
- Unit test: Index entry serialization round-trips
- Integration test: Create session on machine A, resume on machine B via Aspen
- Test: Session append works when Aspen is down (local-only mode)
- Test: Debounce logic — multiple rapid appends result in single upload

---

## Phase 4: Coordination (Distributed Locking)

**Goal**: Cross-machine worktree locking and resource coordination.

**Depends on**: Phase 0

**Complexity**: Small (~1-2 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/coordination.rs` | `DistributedLock` wrapper — acquire/release/renew for worktree locks |

### Files to modify

| File | Change |
|------|--------|
| `src/tools/delegate.rs` (or worktree management code) | Before creating/entering a worktree, acquire a distributed lock if Aspen is configured |
| `src/main.rs` | Register the holder_id (hostname + PID or instance UUID) for lock acquisition |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `LockAcquire { key: "clankers/worktree/{repo}/{branch}", holder_id, ttl_ms, timeout_ms }` | Exclusive worktree lock |
| `LockRelease { key, holder_id, fencing_token }` | Release on session end |
| `LockRenew { key, holder_id, fencing_token, ttl_ms }` | Extend while session is active |
| `LockTryAcquire` | Non-blocking check before entering a worktree |
| `LeaseGrant { ttl_seconds }` | Session-scoped lease that auto-releases locks if process crashes |
| `LeaseKeepalive { lease_id }` | Periodic keepalive during long sessions |

### Design

**Lock key convention**:
```
clankers/worktree/{repo_path_hash}/{worktree_name}
```

**Session-scoped leases**: On startup, grant a lease (TTL: 5 minutes). Attach
all locks to this lease. Run a background keepalive task every 2 minutes. If
the process crashes, locks auto-expire after TTL.

**Degradation**: If Aspen is unreachable, worktrees work without locks (current
behavior). Log a warning.

### Testing strategy

- Unit test: Lock key generation from repo path
- Integration test: Two processes contend for same worktree lock, second blocks then acquires after first releases
- Test: Lease expiry releases locks automatically

---

## Phase 5: Usage Tracking Sync

**Goal**: Token usage/cost data synced to Aspen for centralized accounting
across team members.

**Depends on**: Phase 0

**Complexity**: Small (~1-2 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/usage_sync.rs` | `UsageSyncService` — push daily usage to Aspen KV, aggregate across instances |

### Files to modify

| File | Change |
|------|--------|
| `src/db/usage.rs` | After `record()`, optionally push to Aspen. Add `sync_to_aspen()` method. |
| `src/agent/mod.rs` | After each turn's usage recording, fire async sync |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `ReadKey { key: "clankers/usage/{date}/{instance_id}" }` | Read this instance's usage for today |
| `WriteKey { key: "clankers/usage/{date}/{instance_id}", value }` | Write updated daily usage |
| `ScanKeys { prefix: "clankers/usage/{date}/" }` | Aggregate all instances' usage for a day |
| `CounterAdd { key: "clankers/usage/total_tokens/{date}", amount }` | Atomic team-wide token counter |

### Design

**KV key format**:
```
Per-instance: clankers/usage/{YYYY-MM-DD}/{instance_id}  → DailyUsage
Aggregate:    clankers/usage/total/{YYYY-MM-DD}           → aggregated from scan
```

**Instance ID**: Hostname + stable hash, or configurable in settings.

**Sync**: Fire-and-forget after each `record()` call. No need for CAS — each
instance writes its own key. Aggregation is read-time (scan all instance keys
for a date).

### New slash command: `/team-usage`

Shows aggregated usage across all team members for a date range by scanning
the Aspen KV.

### Testing strategy

- Unit test: Usage record serializes correctly
- Integration test: Two instances record usage, scan aggregates both
- Test: `/team-usage` command displays team totals

---

## Phase 6: Blob Sharing

**Goal**: Large file transfers (code context, build artifacts, session exports)
use Aspen's blob store instead of ad-hoc mechanisms.

**Depends on**: Phase 0

**Complexity**: Small (~1-2 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/blobs.rs` | `BlobService` — upload/download/share files via Aspen blobs |
| `src/tools/aspen_blob.rs` | `AspenBlobTool` — agent tool for sharing files through the cluster |

### Files to modify

| File | Change |
|------|--------|
| `src/tools/mod.rs` | Add `pub mod aspen_blob;` |
| `src/main.rs` | Register `AspenBlobTool` when Aspen is configured |

### Aspen operations used

| Operation | Purpose |
|-----------|---------|
| `AddBlob { data, tag }` | Upload file content |
| `GetBlob { hash }` | Download by hash |
| `GetBlobTicket { hash }` | Generate shareable ticket |
| `DownloadBlob { ticket, tag }` | Fetch from ticket |
| `ListBlobs { limit }` | Browse stored blobs |
| `HasBlob { hash }` | Check existence before upload |
| `ProtectBlob { hash, tag }` | Prevent GC of important blobs |

### Tool definition

```json
{
  "name": "aspen_blob",
  "description": "Upload/download files to the shared Aspen cluster blob store",
  "input_schema": {
    "type": "object",
    "properties": {
      "action": { "enum": ["upload", "download", "list", "share"] },
      "path": { "type": "string", "description": "Local file path (for upload/download)" },
      "hash": { "type": "string", "description": "Blob hash (for download/share)" },
      "tag": { "type": "string", "description": "Protection tag" }
    },
    "required": ["action"]
  }
}
```

### Testing strategy

- Unit test: Upload/download round-trip
- Integration test: Upload file, get ticket, download from ticket on different client
- Test: Large file (>1MB) handles chunking correctly

---

## Phase 7: Auth Bridge

**Goal**: Clankers-auth UCAN tokens work seamlessly with Aspen cluster auth.

**Depends on**: Phase 0, Phase 2

**Complexity**: Medium (~2-3 days)

### Files to create

| File | Purpose |
|------|---------|
| `crates/clankers-aspen/src/auth.rs` | `AuthBridge` — convert between clankers-auth and aspen-auth token formats, token refresh |

### Files to modify

| File | Change |
|------|--------|
| `crates/clankers-auth/src/lib.rs` | Add `to_aspen_token()` method if token formats diverge. Add `from_aspen_token()` for incoming tokens. |
| `crates/clankers-aspen/src/connection.rs` | Use `AuthBridge` to attach tokens to all Aspen requests |

### Design

Both clankers-auth and aspen-auth share the same `CapabilityToken` struct from
`aspen-auth`. The bridge needs to:

1. **Token issuance**: If the user has a clankers identity (SecretKey), request
   a scoped UCAN from the Aspen cluster that grants `clankers/*` key prefix
   access.
2. **Token caching**: Cache the Aspen-issued token locally, refresh before expiry.
3. **Capability mapping**: Map clankers operations to Aspen capabilities:
   - Memory read/write → `Operation::Read/Write { key: "clankers/memory/*" }`
   - Session sync → `Operation::Read/Write { key: "clankers/session/*" }`
   - Blob upload → `Operation::BlobWrite`

### Risks

- Token format drift between clankers-auth and aspen-auth forks. Mitigation:
  Pin to the same version or use aspen-auth directly.

### Testing strategy

- Unit test: Token serialization compatibility between both crates
- Unit test: Capability mapping covers all clankers operations
- Integration test: Authenticated request succeeds, unauthenticated is rejected

---

## Phase 8: Aspen Tools (Forge, CI/CD, Jobs)

**Goal**: Agent can interact with Aspen's forge, CI/CD, and job queue through
new tools.

**Depends on**: Phase 0, Phase 7

**Complexity**: Large (~4-5 days, parallelizable across tools)

### Files to create

| File | Purpose |
|------|---------|
| `src/tools/aspen_forge.rs` | `AspenForgeTool` — git operations (list repos, create issues, patches) |
| `src/tools/aspen_ci.rs` | `AspenCiTool` — trigger pipelines, get status, view logs |
| `src/tools/aspen_jobs.rs` | `AspenJobsTool` — submit/list/cancel background jobs |
| `src/tools/aspen_kv.rs` | `AspenKvTool` — direct KV read/write for power users |
| `src/tools/aspen_coordination.rs` | `AspenCoordinationTool` — locks, counters, sequences (exposed to agent) |

### Files to modify

| File | Change |
|------|--------|
| `src/tools/mod.rs` | Add module declarations for all new tools |
| `src/main.rs` | Conditionally register Aspen tools when cluster is connected |
| `src/agent/system_prompt.rs` | Add Aspen cluster capabilities to system prompt when connected |

### Aspen operations per tool

**AspenForgeTool**:

| Operation | Action |
|-----------|--------|
| `ForgeListRepos` | List repositories |
| `ForgeGetRepo` | Get repo details |
| `ForgeCreateIssue` | Create issue on a repo |
| `ForgeListIssues` | List issues |
| `ForgeCommentIssue` | Add comment to issue |
| `ForgeCreatePatch` | Create PR-equivalent |
| `ForgeListPatches` | List open patches |
| `ForgeLog` | View commit history |
| `ForgeGetCommit` | Get commit details |

**AspenCiTool**:

| Operation | Action |
|-----------|--------|
| `CiTriggerPipeline` | Trigger a CI pipeline |
| `CiGetStatus` | Check pipeline/job status |
| `CiGetJobLogs` | View job logs |
| `CiListRuns` | List recent CI runs |
| `CiCancelRun` | Cancel a running pipeline |

**AspenJobsTool**:

| Operation | Action |
|-----------|--------|
| `JobSubmit` | Submit a background job |
| `JobGet` | Check job status |
| `JobList` | List jobs |
| `JobCancel` | Cancel a job |

### Tool implementation pattern

Each tool follows the existing clankers pattern (see `src/tools/bash.rs`,
`src/tools/web.rs`):

```rust
pub struct AspenForgeTool {
    definition: ToolDefinition,
    connection: Arc<AspenConnection>,
}

#[async_trait]
impl Tool for AspenForgeTool {
    fn definition(&self) -> &ToolDefinition { &self.definition }

    async fn execute(&self, ctx: &ToolContext, params: Value) -> ToolResult {
        let action = params["action"].as_str().unwrap_or("");
        match action {
            "list_repos" => { /* ForgeListRepos */ }
            "create_issue" => { /* ForgeCreateIssue */ }
            _ => ToolResult::error(format!("Unknown action: {action}"))
        }
    }
}
```

### Testing strategy

- Unit test: Each tool's parameter parsing and response formatting
- Integration test: Create repo → create issue → list issues → close issue
- Integration test: Trigger CI → poll status → get logs
- Test: Tool returns helpful error when Aspen is unreachable

---

## Cross-Cutting Concerns

### Error handling

All Aspen operations use `snafu` errors that propagate through
`clankers-aspen`. Every sync operation has a "local-first" fallback — if the
RPC fails, the local operation succeeds and the sync is retried later or
skipped with a warning.

### Observability

- Use `tracing` spans for all Aspen RPCs: `aspen.rpc.{operation}` with
  duration, success/failure
- Optionally push traces to Aspen's own observability service
- Add `/aspen` slash command showing: connection status, last sync times,
  pending syncs, cluster health

### Key namespace governance

All clankers data in Aspen KV lives under the `clankers/` prefix:

```
clankers/memory/{id}                          # Phase 1
clankers/session_index/{cwd_hash}/{sess_id}   # Phase 3
clankers/usage/{date}/{instance_id}           # Phase 5
clankers/worktree/{repo_hash}/{branch}        # Phase 4 (locks)
clankers/config/{instance_id}                 # Future: shared config
```

### Offline resilience

Every phase must handle `AspenConnection::send()` failures gracefully:
1. Log warning
2. Fall back to local-only behavior
3. Queue the operation for retry (bounded queue, drop oldest on overflow)
4. Resume sync when connection recovers

### Migration path

No migration needed — Aspen integration is purely additive. If
`settings.aspen` is not configured, behavior is identical to current clankers.
First time Aspen is configured, initial sync populates the cluster from local
data.

---

## Estimated Effort

| Phase | Complexity | New files | Modified files | Est. effort |
|-------|-----------|-----------|----------------|-------------|
| 0: Foundation | Medium | 5 | 3 | 2-3 days |
| 1: Memory Sync | Medium | 1 | 3 | 2-3 days |
| 2: Secrets | Small | 1 | 3 | 1-2 days |
| 3: Session Sync | Large | 2 | 3 | 4-5 days |
| 4: Coordination | Small | 1 | 2 | 1-2 days |
| 5: Usage Sync | Small | 1 | 2 | 1-2 days |
| 6: Blob Sharing | Small | 2 | 2 | 1-2 days |
| 7: Auth Bridge | Medium | 1 | 2 | 2-3 days |
| 8: Tools | Large | 5 | 3 | 4-5 days |
| **Total** | | **19** | **~15 unique** | **~18-27 days** |

---

## Open Questions & Risks

1. **iroh version alignment**: Clankers uses iroh 0.96, aspen-client uses
   0.95.1. May cause build conflicts. Check `Cargo.lock` compatibility before
   starting. May need to align iroh versions.

2. **Session JSONL refactor scope**: Extracting `SessionStore` trait from the
   current free-function-based `store.rs` touches the core session loop. This
   is the riskiest refactor. Do Phase 3 last among the core phases, after
   gaining confidence with simpler integrations.

3. **Aspen cluster availability during development**: Need a running Aspen
   cluster for integration tests. Use `aspen-testing` crate's in-process test
   cluster if available, or set up a single-node dev cluster.

4. **Blob size limits**: `MAX_CLIENT_MESSAGE_SIZE` is 4MB for Aspen. Long
   sessions could exceed this. Use `AddBlob` for small data, chunked transfer
   for large sessions, or compress before upload.

5. **KV key length limits**: Aspen may have key length limits. CWD paths
   encoded into keys could be long. Use BLAKE3 hash of CWD path in keys
   instead of encoded path.
