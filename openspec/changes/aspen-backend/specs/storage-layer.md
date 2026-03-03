# Storage Layer — aspen KV as clankers backend

## Summary

Replace clankers' direct redb usage with a `ClankerStorage` trait that has
two implementations: `RedbStorage` (current, local-only) and `AspenStorage`
(distributed, Raft-replicated via aspen-client).  The active implementation
is selected at startup based on configuration.

## Key Schema

All clankers keys live under the `clankers:` prefix to isolate from other
aspen layers running on the same cluster.

### Sessions

```
clankers:sessions:{id}:meta
  → { "id", "created", "updated", "model", "mode", "label", "turn_count", "node_id" }

clankers:sessions:{id}:turns:{seq:08}
  → { "seq", "role", "content", "tool_calls", "tool_results", "timestamp", "tokens" }

clankers:sessions:{id}:system_prompt
  → the system prompt text (versioned, CAS-guarded)
```

The `seq` field is zero-padded to 8 digits for lexicographic ordering via
aspen's prefix scan.  This supports up to 99,999,999 turns per session.

### Config

```
clankers:config:global:{key}       → cluster-wide settings
clankers:config:node:{node-id}:{key} → per-node overrides
```

Global config is readable by all nodes.  Per-node config takes precedence
when present (local override pattern).  Config keys include:

- `default_model` — default model for new sessions
- `system_prompt` — default system prompt
- `max_sessions` — daemon session limit
- `provider:{name}:api_key` — provider credentials (encrypted at rest)
- `tools:disabled` — JSON array of globally disabled tool names

### Usage

```
clankers:usage:{YYYY-MM-DD}:{node-id}:{uuid}
  → { "model", "provider", "input_tokens", "output_tokens", "cache_read",
      "cache_write", "cost_usd", "session_id", "timestamp" }
```

Date prefix enables efficient daily/monthly aggregation via scan.

### Audit

```
clankers:audit:{timestamp-ms}:{node-id}:{uuid}
  → { "event", "user", "session_id", "details", "timestamp" }
```

Timestamp-first ordering for chronological queries.

## ClankerStorage Trait

```rust
#[async_trait]
pub trait ClankerStorage: Send + Sync + 'static {
    // Sessions
    async fn save_session_meta(&self, id: &str, meta: &SessionMeta) -> Result<()>;
    async fn load_session_meta(&self, id: &str) -> Result<Option<SessionMeta>>;
    async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<SessionMeta>>;
    async fn delete_session(&self, id: &str) -> Result<()>;
    async fn append_turn(&self, session_id: &str, turn: &TurnEntry) -> Result<u64>;
    async fn load_turns(&self, session_id: &str, from_seq: u64, limit: usize) -> Result<Vec<TurnEntry>>;

    // Config
    async fn get_config(&self, key: &str) -> Result<Option<String>>;
    async fn set_config(&self, key: &str, value: &str) -> Result<()>;
    async fn delete_config(&self, key: &str) -> Result<()>;

    // Usage
    async fn record_usage(&self, entry: &UsageEntry) -> Result<()>;
    async fn query_usage(&self, filter: &UsageFilter) -> Result<Vec<UsageEntry>>;

    // Audit
    async fn append_audit(&self, entry: &AuditEntry) -> Result<()>;
    async fn query_audit(&self, filter: &AuditFilter) -> Result<Vec<AuditEntry>>;
}
```

## AspenStorage Implementation

```rust
pub struct AspenStorage {
    client: AspenClient,
    node_id: String,
}

impl AspenStorage {
    pub async fn connect(ticket: &str) -> Result<Self> { ... }
}

#[async_trait]
impl ClankerStorage for AspenStorage {
    async fn append_turn(&self, session_id: &str, turn: &TurnEntry) -> Result<u64> {
        // Atomic: read current turn count via CAS, write new turn
        let meta_key = format!("clankers:sessions:{}:meta", session_id);
        let mut meta = self.client.get(&meta_key).await?...;
        let seq = meta.turn_count;
        let turn_key = format!("clankers:sessions:{}:turns:{:08}", session_id, seq);

        // Batch write: turn entry + updated meta (atomic via aspen BatchWrite)
        self.client.batch_write(vec![
            WriteOp::Set(turn_key, serde_json::to_string(turn)?),
            WriteOp::Cas(meta_key, old_revision, updated_meta),
        ]).await?;

        Ok(seq)
    }

    async fn list_sessions(&self, limit: usize, offset: usize) -> Result<Vec<SessionMeta>> {
        // Prefix scan over clankers:sessions:*:meta
        let results = self.client.scan("clankers:sessions:", limit + offset).await?;
        // Filter for :meta suffix, skip offset, take limit
        ...
    }
}
```

## Migration

```rust
/// Migrate local redb data to aspen cluster.
pub async fn migrate_to_aspen(
    local: &RedbStorage,
    remote: &AspenStorage,
) -> Result<MigrationReport> {
    let mut report = MigrationReport::default();

    // Sessions
    for session in local.list_sessions(usize::MAX, 0).await? {
        remote.save_session_meta(&session.id, &session).await?;
        let turns = local.load_turns(&session.id, 0, usize::MAX).await?;
        for turn in turns {
            remote.append_turn(&session.id, &turn).await?;
        }
        report.sessions += 1;
    }

    // Config
    for (key, value) in local.all_config().await? {
        remote.set_config(&key, &value).await?;
        report.config_keys += 1;
    }

    report
}
```

## Concurrency

Session writes are serialized per-session using aspen's `DistributedLock`:

```
Lock key: clankers:locks:session:{session-id}
TTL: 300s (5 minute lease, renewed on activity)
```

This replaces the current per-session `Mutex<()>` in the daemon's
`SessionStore`, which only works within a single process.

## Consistency

- All session writes are **linearizable** (Raft-backed).
- Turn append uses **CAS** on the session meta to detect concurrent writers.
- Config reads can use **stale reads** for performance (most config is
  read-heavy, write-rarely).
- Usage/audit writes are **fire-and-forget** batched writes (aspen's write
  batching amortizes fsync cost across many concurrent entries).
