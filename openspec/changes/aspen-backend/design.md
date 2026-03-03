# aspen-backend — Design

## Decisions

### Stateless layer, not embedded database

**Choice:** Clankers stores all persistent state in aspen's KV and blob
stores.  No local redb database when running in cluster mode.
**Rationale:** This is aspen's core design philosophy — the FoundationDB
"layer" pattern.  Every feature above the KV + blob primitives is stateless.
Clankers sessions, configs, and audit logs are just KV entries with
structured prefixes.  This gives us replication, failover, and multi-node
access for free.
**Alternatives considered:** Embed an aspen-raft node directly inside
clankers (too heavy, clankers becomes a consensus participant).  Keep redb
and sync it via CRDT (complex, eventual consistency for sessions is
confusing).

### Dual-mode: standalone vs. cluster

**Choice:** Clankers detects an `aspen_cluster` config entry at startup.
If present, connect to the cluster via aspen-client.  If absent, use local
redb as today.  A `StorageBackend` trait abstracts both.
**Rationale:** Most users run a single clankers instance on their laptop.
Forcing an aspen cluster would be a terrible UX cliff.  The standalone path
must remain zero-config.  The cluster path is opt-in for teams, CI farms,
and daemon deployments that want replication.
**Alternatives considered:** Always embed a single-node aspen cluster
(adds startup cost and complexity even for single-user).  Require explicit
`--standalone` flag (bad default).

### Share aspen's iroh endpoint, don't create a second one

**Choice:** When running in cluster mode, clankers registers its ALPNs
(`clankers/rpc/1`, `clankers/chat/1`) on aspen's existing iroh endpoint
via the `ProtocolRegistry`.
**Rationale:** Aspen already manages the QUIC endpoint, NAT traversal,
relay connections, and peer discovery.  Creating a second endpoint wastes
a port, duplicates relay traffic, and complicates firewall rules.  ALPN
multiplexing is designed exactly for this.
**Alternatives considered:** Separate endpoint (wasteful).  Proxy through
aspen's RPC layer (adds latency to every message, agent streaming suffers).

### aspen-auth replaces both allowlist AND planned clankers-auth

**Choice:** Drop the planned `clankers-auth` crate (from the ucan-auth
openspec).  Use aspen-auth directly with clankers-specific capability types
registered as a custom capability namespace.
**Rationale:** The ucan-auth proposal was already planning to fork aspen-auth.
If aspen IS the backend, we just use it directly — no fork needed.  Aspen's
UCAN system supports custom capability types via the `Capability` enum.
We add clankers-specific variants: `Prompt`, `ToolUse { tools }`,
`BotCommand`, `SessionManage`, `ModelSwitch`, `FileAccess { paths }`.
**Alternatives considered:** Keep the fork plan (duplicate effort, types
diverge, maintenance burden doubles).

### Hyperlight replaces Extism for WASM plugins

**Choice:** Migrate from Extism to Hyperlight-wasm for plugin hosting.
**Rationale:** Aspen already uses Hyperlight with sandboxed host functions,
KV namespace isolation, capability-based permissions, and hot reload via
ArcSwap.  Extism is a simpler runtime but lacks these features.  The
clankers plugin ABI (`handle_tool_call` → JSON in/out) maps directly to
Hyperlight's guest function model.  Existing plugin WASM binaries need
recompilation but the Rust source is almost identical.
**Alternatives considered:** Keep Extism alongside Hyperlight (two WASM
runtimes is confusing).  Wrap Extism behind aspen's plugin registry (would
defeat the purpose of unification).

### Subagents as distributed jobs, not local tokio tasks

**Choice:** When running in cluster mode, subagent delegation uses
aspen-jobs instead of spawning local tokio tasks.
**Rationale:** Aspen-jobs provides priority queues, retry with backoff,
dead letter queues, worker affinity, and distributed execution across
cluster nodes.  A subagent task is just a job with type `"agent-prompt"`,
a payload of the prompt + tools + model, and a result of the agent's
response.  This lets a 4-node clankers cluster run 4× the subagents
concurrently, with automatic retry if a node goes down mid-turn.
**Alternatives considered:** Keep local-only (no cluster benefits).
Custom work-stealing queue (reinvents aspen-jobs).

### KV key schema follows aspen conventions

**Choice:** All clankers data lives under a `clankers:` KV prefix,
with subprefixes for each data domain.
**Rationale:** Aspen's KV uses hierarchical string keys with `:`
separators (same as etcd/Consul).  This keeps clankers' data isolated
from aspen's internal keys and any other layers running on the same
cluster.

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                     clankers binary                              │
│                                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────────┐    │
│  │   TUI    │  │ Headless │  │  Daemon  │  │  CLI / REPL  │    │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘  └──────┬───────┘    │
│       │              │             │               │            │
│  ┌────▼──────────────▼─────────────▼───────────────▼────────┐   │
│  │                    Agent Layer                            │   │
│  │  agent, tools, system_prompt, provider, streaming         │   │
│  └────────────────────────┬─────────────────────────────────┘   │
│                           │                                     │
│  ┌────────────────────────▼─────────────────────────────────┐   │
│  │              clankers-aspen bridge                         │   │
│  │                                                           │   │
│  │  SessionStore ──→ aspen KV   (clankers:sessions:*)        │   │
│  │  ConfigStore  ──→ aspen KV   (clankers:config:*)          │   │
│  │  UsageStore   ──→ aspen KV   (clankers:usage:*)           │   │
│  │  AuditStore   ──→ aspen KV   (clankers:audit:*)           │   │
│  │  BlobStore    ──→ iroh-blobs (file attachments, outputs)  │   │
│  │  AuthVerifier ──→ aspen-auth (UCAN tokens)                │   │
│  │  JobDispatch  ──→ aspen-jobs (subagent work)              │   │
│  │  PluginHost   ──→ aspen WASM (Hyperlight plugins)         │   │
│  │  Locks/Semas  ──→ aspen-coordination (agent exclusion)    │   │
│  └───────────────────────┬──────────────────────────────────┘   │
│                          │                                      │
│         ┌────────────────┼────────────────┐                     │
│         │ Standalone     │ Cluster         │                     │
│         │ (redb local)   │ (aspen-client)  │                     │
│         └────────────────┴────────────────┘                     │
│                          │                                      │
└──────────────────────────┼──────────────────────────────────────┘
                           │
               ┌───────────▼───────────┐
               │    Aspen Cluster      │
               │                       │
               │  Raft (KV + metadata) │
               │  iroh-blobs (P2P)     │
               │  Jobs (workers)       │
               │  Auth (UCAN)          │
               │  Coordination         │
               └───────────────────────┘
```

### Storage Backend Trait

```rust
/// Abstraction over local (redb) and distributed (aspen) storage.
#[async_trait]
pub trait ClankerStorage: Send + Sync {
    // ── Sessions ──
    async fn save_session(&self, id: &str, entry: &SessionEntry) -> Result<()>;
    async fn load_session(&self, id: &str) -> Result<Option<SessionEntry>>;
    async fn list_sessions(&self, limit: usize) -> Result<Vec<SessionSummary>>;
    async fn delete_session(&self, id: &str) -> Result<()>;

    // ── Config ──
    async fn get_config(&self, key: &str) -> Result<Option<String>>;
    async fn set_config(&self, key: &str, value: &str) -> Result<()>;

    // ── Usage ──
    async fn record_usage(&self, entry: &UsageEntry) -> Result<()>;
    async fn query_usage(&self, filter: &UsageFilter) -> Result<Vec<UsageEntry>>;

    // ── Audit ──
    async fn append_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn query_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>>;

    // ── Blobs ──
    async fn store_blob(&self, data: &[u8]) -> Result<BlobRef>;
    async fn fetch_blob(&self, blob_ref: &BlobRef) -> Result<Vec<u8>>;
}
```

### KV Key Schema

```
clankers:sessions:{session-id}:meta          → SessionMetadata (JSON)
clankers:sessions:{session-id}:turns:{seq}   → TurnEntry (JSON)
clankers:sessions:{session-id}:index         → SessionIndex (for search)

clankers:config:{node-id}:{key}              → config value
clankers:config:global:{key}                 → cluster-wide config

clankers:usage:{date}:{node-id}:{seq}        → UsageEntry (JSON)

clankers:audit:{timestamp}:{node-id}:{seq}   → AuditEntry (JSON)

clankers:plugins:{name}:manifest             → PluginManifest (JSON)
clankers:plugins:{name}:config               → plugin config

clankers:agents:{daemon-id}:status           → agent status + metadata
clankers:agents:{daemon-id}:sessions         → active session count

clankers:router:{node-id}:providers          → provider registry
clankers:router:{node-id}:circuit-breaker    → per-provider health state
clankers:router:cache:{hash}                 → cached LLM responses
```

### ALPN Registration (Cluster Mode)

```
Aspen's existing ALPN table:
  raft-auth           → Raft consensus
  aspen-client        → Client RPC
  iroh-blobs/0        → Blob transfer
  iroh-gossip/0       → Peer discovery

Clankers registers:
  clankers/rpc/1      → JSON-RPC (ping, status, prompt, file)
  clankers/chat/1     → Conversational sessions
  clankers/stream/1   → LLM response streaming (new)
```

### Auth Integration

```
┌──────────────────────────────────────────────────────────┐
│                   aspen-auth                             │
│                                                          │
│  Existing capabilities:                                  │
│    Full, Read, Write, Delete, Delegate                   │
│                                                          │
│  New clankers namespace (registered via plugin API):     │
│    Prompt           — can send prompts to the agent      │
│    ToolUse { tools }— can use specific tools             │
│    BotCommand       — can issue !status, !restart, etc.  │
│    SessionManage    — can list/delete/resume sessions     │
│    ModelSwitch      — can change the active model         │
│    FileAccess       — can read/write files via tools      │
│                                                          │
│  Token flow unchanged from aspen:                        │
│    Owner signs token → user presents token → verify      │
│    Delegation: user creates child token with ⊆ caps      │
└──────────────────────────────────────────────────────────┘
```

### Job-Based Subagents

```
Clankers agent decides to delegate work
  │
  ├─ Standalone mode: spawn local tokio task (current behavior)
  │
  └─ Cluster mode: submit aspen job
      │
      ├─ Job type: "clankers-agent-prompt"
      ├─ Payload: { prompt, tools, model, system_prompt, context }
      ├─ Priority: Normal (or High for user-facing delegation)
      ├─ Worker affinity: any node with clankers agent capability
      │
      ├─ aspen-jobs routes to available worker node
      ├─ Worker node creates ephemeral Agent, runs prompt
      ├─ Result stored as job output
      │
      └─ Originating node reads job result, continues conversation
```

## Data Flow

### Startup (cluster mode)

```
clankers --cluster aspen://ticket-or-node-id
  │
  ├─ Parse cluster ticket / resolve node ID
  ├─ Create aspen-client connection
  ├─ Authenticate with UCAN token (or cluster cookie for bootstrap)
  ├─ Register clankers ALPNs on shared endpoint
  ├─ Announce capabilities via gossip:
  │    { "type": "clankers-daemon", "model": "claude-sonnet-4-5", "tools": [...] }
  ├─ Load config from clankers:config:*
  ├─ Resume any interrupted sessions from clankers:sessions:*
  └─ Enter mode (TUI / headless / daemon)
```

### Prompt execution (cluster mode)

```
User sends prompt (via TUI, headless, daemon, iroh, Matrix)
  │
  ├─ Auth check: verify UCAN token against aspen-auth
  │   └─ Extract capabilities → filter tools
  │
  ├─ Session lookup: get/create in aspen KV
  │   └─ clankers:sessions:{id}:meta (CAS for exclusive access)
  │
  ├─ Acquire distributed lock: clankers:locks:session:{id}
  │   └─ Prevents concurrent prompts to same session
  │
  ├─ Build agent with filtered tools
  ├─ Stream LLM response (provider layer unchanged)
  │
  ├─ On each tool call:
  │   ├─ Verify capability
  │   ├─ Execute tool
  │   └─ If blob output → store in iroh-blobs, record hash
  │
  ├─ On subagent delegation:
  │   ├─ Submit aspen job (cluster) or spawn task (standalone)
  │   └─ Wait for result, inject into conversation
  │
  ├─ Write turn to KV: clankers:sessions:{id}:turns:{seq}
  ├─ Record usage:    clankers:usage:{date}:{node}:{seq}
  ├─ Append audit:    clankers:audit:{ts}:{node}:{seq}
  │
  └─ Release distributed lock
```

### Plugin migration

```
Extism plugin (current)                    Hyperlight plugin (target)
─────────────────────                      ──────────────────────────
plugin.json + .wasm                        KV manifest + blob store WASM
handle_tool_call(JSON) → JSON              handle_tool_call(JSON) → JSON
No KV access                               Host functions: kv_get/set/delete
No capability model                        PluginPermissions (least privilege)
No hot reload                              ArcSwap hot reload
extism-pdk guest SDK                       clankers-plugin-sdk (adapted)
```

### Standalone ↔ cluster migration

```
User starts with standalone (redb), later wants cluster:

  clankers migrate --to aspen://ticket
    │
    ├─ Connect to aspen cluster
    ├─ Scan local redb sessions → write to clankers:sessions:*
    ├─ Scan local redb config → write to clankers:config:*
    ├─ Scan local redb usage → write to clankers:usage:*
    ├─ Copy blob files → store in iroh-blobs
    └─ Print migration summary, prompt to switch config
```
