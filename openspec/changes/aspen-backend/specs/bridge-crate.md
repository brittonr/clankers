# Bridge Crate — clankers-aspen

## Summary

A new `crates/clankers-aspen/` crate that connects clankers to an aspen
cluster.  It wraps `aspen-client` and exposes clankers-specific storage,
auth, coordination, and job interfaces.  All other clankers code depends
on trait abstractions — the bridge crate provides the distributed
implementations.

## Crate Structure

```
crates/clankers-aspen/
  Cargo.toml
  src/
    lib.rs              ← re-exports, ClankersAspen builder
    storage.rs          ← AspenStorage impl of ClankerStorage
    auth.rs             ← AspenAuth impl of ClankerAuth
    coordination.rs     ← AspenCoordination impl of ClankerCoordination
    jobs.rs             ← AgentWorker, job submission
    endpoint.rs         ← SharedEndpoint ALPN registration
    blobs.rs            ← Blob storage (iroh-blobs via aspen)
    config.rs           ← ClusterConfig parsing
    migration.rs        ← Redb → aspen KV data migration
```

## Dependencies

```toml
[package]
name = "clankers-aspen"
version = "0.1.0"
edition.workspace = true

[dependencies]
# Aspen client library (the only aspen dep needed at runtime)
aspen-client = { path = "../../aspen/crates/aspen-client" }
aspen-auth = { path = "../../aspen/crates/aspen-auth" }
aspen-coordination = { path = "../../aspen/crates/aspen-coordination" }
aspen-jobs = { path = "../../aspen/crates/aspen-jobs" }
aspen-client-api = { path = "../../aspen/crates/aspen-client-api" }

# Clankers core traits
clankers-core = { path = "../clankers-core" }  # new: extracted trait crate

# Standard deps
tokio = { version = "1", features = ["full"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
iroh = { version = "0.96" }
tracing = "0.1"
async-trait = "0.1"
snafu = "0.8"
```

## ClankersAspen Builder

```rust
/// Main entry point for connecting clankers to an aspen cluster.
pub struct ClankersAspen {
    client: AspenClient,
    storage: AspenStorage,
    auth: AspenAuth,
    coordination: AspenCoordination,
    job_manager: AspenJobManager,
    endpoint: SharedEndpoint,
}

impl ClankersAspen {
    /// Connect to an aspen cluster.
    pub async fn connect(config: &ClusterConfig) -> Result<Self> {
        let client = match &config.connection {
            // Cluster ticket (compact bootstrap info)
            ConnectionInfo::Ticket(ticket) => {
                AspenClient::from_ticket(ticket).await?
            }
            // Direct node ID
            ConnectionInfo::NodeId(id) => {
                AspenClient::connect(*id).await?
            }
            // Cookie-based auth for bootstrap
            ConnectionInfo::Cookie { nodes, cookie } => {
                AspenClient::connect_with_cookie(nodes, cookie).await?
            }
        };

        let storage = AspenStorage::new(client.clone());
        let auth = AspenAuth::new(client.clone());
        let coordination = AspenCoordination::new(client.clone());
        let job_manager = AspenJobManager::new(client.clone());
        let endpoint = SharedEndpoint::register(&client).await?;

        Ok(Self { client, storage, auth, coordination, job_manager, endpoint })
    }

    /// Get the storage backend.
    pub fn storage(&self) -> &AspenStorage { &self.storage }

    /// Get the auth verifier.
    pub fn auth(&self) -> &AspenAuth { &self.auth }

    /// Get coordination primitives.
    pub fn coordination(&self) -> &AspenCoordination { &self.coordination }

    /// Get the job manager for subagent dispatch.
    pub fn jobs(&self) -> &AspenJobManager { &self.job_manager }

    /// Get the shared iroh endpoint.
    pub fn endpoint(&self) -> &SharedEndpoint { &self.endpoint }

    /// Graceful shutdown.
    pub async fn shutdown(&self) -> Result<()> {
        self.endpoint.cancel.cancel();
        self.client.close().await?;
        Ok(())
    }
}
```

## Configuration

```toml
# ~/.clankers/config.toml (new fields)

[cluster]
# One of: ticket, node_id, or cookie-based
ticket = "aspen{...}"
# node_id = "abc123..."
# cookie = "my-cluster"
# nodes = ["nodeA", "nodeB"]

# Storage mode: "local" (redb), "cluster" (aspen), "hybrid" (local + sync)
storage = "cluster"

# Register as a worker for subagent jobs
worker = true
worker_capabilities = ["claude-sonnet-4-5", "all-tools"]
```

## Trait Extraction — clankers-core

To avoid circular dependencies, common traits are extracted into a new
`crates/clankers-core/` crate that both the main binary and bridge crate
depend on:

```rust
// crates/clankers-core/src/lib.rs

/// Storage trait — implemented by RedbStorage and AspenStorage
#[async_trait]
pub trait ClankerStorage: Send + Sync + 'static { ... }

/// Auth trait — implemented by AllowlistAuth and AspenAuth
#[async_trait]
pub trait ClankerAuth: Send + Sync + 'static {
    async fn verify(&self, sender: &SenderId) -> Result<AuthResult>;
    async fn create_token(&self, caps: Vec<ClankerCapability>, ttl: Duration) -> Result<String>;
    async fn revoke_token(&self, hash: &str) -> Result<()>;
}

/// Coordination trait — implemented by LocalCoordination and AspenCoordination
#[async_trait]
pub trait ClankerCoordination: Send + Sync + 'static {
    async fn acquire_session_lock(&self, session_id: &str) -> Result<Box<dyn LockGuard>>;
    async fn try_acquire_agent_permit(&self) -> Result<Option<Box<dyn PermitGuard>>>;
    async fn check_rate_limit(&self, user_id: &str) -> Result<bool>;
}

/// Job dispatch trait — implemented by LocalDispatch and AspenJobDispatch
#[async_trait]
pub trait ClankerJobDispatch: Send + Sync + 'static {
    async fn submit_agent_job(&self, payload: AgentPromptPayload) -> Result<JobHandle>;
    async fn cancel_job(&self, job_id: &str) -> Result<()>;
}
```

## Backend Assembly

At startup, clankers assembles the appropriate backend:

```rust
// src/main.rs (simplified)
async fn main() {
    let config = load_config()?;

    let backend: Arc<dyn ClankerBackend> = if let Some(cluster) = &config.cluster {
        // Cluster mode — connect to aspen
        let aspen = ClankersAspen::connect(cluster).await?;
        Arc::new(aspen)
    } else {
        // Standalone mode — local redb
        let local = LocalBackend::new(&config.data_dir)?;
        Arc::new(local)
    };

    // Agent, TUI, daemon all use the abstract backend
    run_mode(config.mode, backend).await?;
}
```

## Feature Flag

The bridge crate is behind a cargo feature to keep the standalone build
lean:

```toml
# Root Cargo.toml
[features]
default = []
cluster = ["dep:clankers-aspen"]

[dependencies]
clankers-aspen = { path = "crates/clankers-aspen", optional = true }
```

Users who don't need cluster mode pay zero cost — no aspen dependencies
in the binary.
