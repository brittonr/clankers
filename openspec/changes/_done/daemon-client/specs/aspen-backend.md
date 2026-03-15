# Aspen Backend

## Purpose

Define how an aspen cluster serves as an optional backend for clankers,
replacing local file storage with distributed KV, blobs, and job
execution. This is additive — local mode remains the default.

## Requirements

### SessionBackend trait

The system MUST define a `SessionBackend` trait that abstracts session
persistence. Both local and aspen backends implement it.

```rust
#[async_trait]
trait SessionBackend: Send + Sync {
    /// Write a session entry
    async fn append_entry(&self, session_id: &str, entry: &SessionEntry) -> Result<()>;

    /// Load all entries for a session
    async fn load_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>>;

    /// List all sessions
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;

    /// Delete a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Save the Automerge document (for CRDT-backed sessions)
    async fn save_document(&self, session_id: &str, doc: &[u8]) -> Result<()>;

    /// Load the Automerge document
    async fn load_document(&self, session_id: &str) -> Result<Option<Vec<u8>>>;

    /// Store a blob (tool output, image, file artifact)
    async fn store_blob(&self, data: &[u8]) -> Result<String>;  // returns hash/ID

    /// Retrieve a blob
    async fn get_blob(&self, id: &str) -> Result<Option<Vec<u8>>>;
}
```

### Local backend

The default `LocalSessionBackend` MUST implement `SessionBackend` using
local files.

- `append_entry` → append to Automerge doc + save to `.automerge` file
- `load_entries` → load Automerge doc, extract entries
- `save_document` → write `.automerge` file to session directory
- `store_blob` → write to `~/.local/share/clankers/blobs/<hash>`

GIVEN no `--backend` flag is set
WHEN the daemon starts
THEN it uses `LocalSessionBackend`
AND sessions are stored in `~/.local/share/clankers/sessions/`

### Aspen backend

The `AspenSessionBackend` MUST implement `SessionBackend` using
aspen's client API.

- `append_entry` → `WriteKey { key: "sessions/{id}/entries/{n}", value }`
- `load_entries` → `ScanKeys { prefix: "sessions/{id}/entries/" }`
- `save_document` → `WriteKey { key: "sessions/{id}/doc", value }` (Automerge bytes)
- `load_document` → `ReadKey { key: "sessions/{id}/doc" }`
- `store_blob` → `AddBlob { data }` (content-addressed by BLAKE3 hash)
- `get_blob` → `GetBlob { hash }`
- `list_sessions` → `ScanKeys { prefix: "sessions/" }` + parse metadata

GIVEN `--backend aspen --ticket <ticket>` is set
WHEN the daemon starts
THEN it connects to the aspen cluster via `AspenClient::connect()`
AND uses `AspenSessionBackend` for all persistence

GIVEN the aspen cluster has 3 nodes
WHEN a session entry is written
THEN it is replicated via Raft consensus
AND any node can serve reads for that session

### Auth unification

The system SHOULD unify `clankers-auth` tokens with `aspen-auth` tokens
since `clankers-auth` was forked from `aspen-auth`.

GIVEN a user has an aspen cluster token with `Full { prefix: "sessions/" }`
WHEN they connect to a clankers daemon backed by aspen
THEN the daemon maps aspen capabilities to clankers capabilities:
  - `Read { prefix: "sessions/" }` → session read access
  - `Write { prefix: "sessions/" }` → session write access
  - aspen `ShellExecute` → clankers `ShellExecute`
  - aspen `Delegate` → clankers `Delegate`

### Distributed agent execution

When using the aspen backend, the system SHOULD support submitting
subagent work as aspen jobs.

```rust
struct AgentJobWorker {
    controller: SessionController,
}

#[async_trait]
impl aspen_jobs::Worker for AgentJobWorker {
    async fn execute(&self, job: Job) -> JobResult {
        let task = job.payload::<AgentTask>()?;
        let cmd = SessionCommand::Prompt {
            text: task.prompt,
            images: vec![],
        };
        self.controller.handle_command(cmd).await?;
        // collect results from DaemonEvent stream
        Ok(JobOutput::success(result))
    }
}
```

GIVEN a parent agent on machine A invokes the subagent tool
WHEN the daemon is backed by aspen
THEN the subagent task is submitted as an aspen job
AND any node in the cluster can pick up and execute the job
AND results flow back through aspen KV

GIVEN the subagent job is picked up by machine B
WHEN the agent on machine B runs the task
THEN tool outputs are stored as aspen blobs
AND the session is written to aspen KV
AND the parent on machine A reads results from KV

### Session sync via iroh-docs (aspen-native)

When backed by aspen, session documents SHOULD sync via aspen's
existing iroh-docs integration rather than a separate iroh-docs
setup.

```
SessionController (daemon)
    │
    ▼ (Automerge changes)
aspen KV (Raft-committed)
    │
    ▼ (DocsExporter)
iroh-docs namespace
    │
    ▼ (P2P sync)
TUI Client (local replica)
```

This reuses aspen's `DocsExporter` pattern: Raft log entries for
session keys are automatically exported to iroh-docs namespaces.
TUI clients subscribe to the namespace for real-time updates.

### Blob storage for tool artifacts

Large tool outputs (file contents, grep results, images) SHOULD be
stored as aspen blobs instead of inline in session entries.

GIVEN a tool produces 500KB of output
WHEN the session entry is written
THEN the output is stored as an aspen blob (content-addressed by BLAKE3)
AND the session entry contains the blob hash, not the full output
AND the TUI fetches the blob on demand for display

This keeps session entries small and KV-friendly while preserving
full tool output in the content-addressed blob store.

### Backend selection

The system MUST support backend selection via CLI flag and config.

```
clankers daemon                              # local backend (default)
clankers daemon --backend aspen --ticket T   # aspen backend
```

Settings:
```json
{
    "backend": "local",
    "aspen_ticket": null
}
```

GIVEN `"backend": "aspen"` in settings
WHEN the daemon starts without a `--backend` flag
THEN it uses the aspen backend with the configured ticket

### Fallback

The system MUST handle aspen cluster unavailability gracefully.

GIVEN the daemon is configured with aspen backend
WHEN the aspen cluster is unreachable
THEN the daemon logs a warning
AND falls back to local backend
AND queues writes for sync when the cluster comes back

This is NOT full offline-first — it's a degradation path. The
Automerge CRDT layer handles true offline-first for session data.
The aspen backend handles durable distributed storage when available.
