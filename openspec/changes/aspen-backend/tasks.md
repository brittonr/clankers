# aspen-backend — Tasks

## Phase 1: Foundation (traits + bridge crate)

- [ ] Extract `crates/clankers-core/` with storage, auth, coordination, blob, and job dispatch traits
- [ ] Wrap existing redb code as `RedbStorage` implementing `ClankerStorage`
- [ ] Wrap existing allowlist as `AllowlistAuth` implementing `ClankerAuth`
- [ ] Wrap existing tokio mutexes as `LocalCoordination` implementing `ClankerCoordination`
- [ ] Wrap existing local task spawning as `LocalDispatch` implementing `ClankerJobDispatch`
- [ ] Wrap existing filesystem blobs as `LocalBlobStore` implementing `ClankerBlobStore`
- [ ] Refactor `Agent`, `SessionStore`, `DaemonConfig` to use trait objects instead of concrete types
- [ ] Add `cluster` feature flag (default off)
- [ ] Verify all existing tests pass with the trait abstraction (no behavior change)

## Phase 2: Bridge crate + storage

- [ ] Create `crates/clankers-aspen/` with `aspen-client` dependency
- [ ] Implement `AspenStorage` (KV-backed sessions, config, usage, audit)
- [ ] Implement KV key schema (`clankers:sessions:*`, `clankers:config:*`, etc.)
- [ ] Implement session turn append with CAS for concurrent writer detection
- [ ] Implement session listing via prefix scan with pagination
- [ ] Implement `ClusterConfig` parsing from `~/.clankers/config.toml`
- [ ] Implement `ClankersAspen::connect()` builder
- [ ] Add integration test: write sessions to aspen KV, read them back
- [ ] Add integration test: concurrent session writes from two clients
- [ ] Implement `clankers migrate --to aspen://ticket` data migration tool

## Phase 3: Shared endpoint

- [ ] Implement `SharedEndpoint` — register clankers ALPNs on aspen's iroh endpoint
- [ ] Implement `EndpointMode` enum (Standalone / Cluster)
- [ ] Wire daemon accept loop to work with either endpoint mode
- [ ] Add gossip announcement with clankers metadata (model, tools, version)
- [ ] Test: clankers daemon reachable via aspen's endpoint (same NodeId)
- [ ] Test: standalone mode still creates its own endpoint (regression)
- [ ] Register `clankers-router/proxy/1` ALPN on shared endpoint

## Phase 4: Auth integration

- [ ] Implement `AspenAuth` wrapping aspen-auth's `TokenVerifier`
- [ ] Define `ClankerCapability` enum (Prompt, ToolUse, BotCommand, etc.)
- [ ] Implement capability containment check for delegation
- [ ] Wire auth verification into daemon message handler
- [ ] Implement `clankers token create/list/revoke/inspect` CLI commands
- [ ] Implement `!token` and `!delegate` bot commands for Matrix
- [ ] Add iroh auth frame support (optional, backwards compatible)
- [ ] Test: token-scoped tool filtering (user only sees allowed tools)
- [ ] Test: delegation creates valid child token with ⊆ capabilities
- [ ] Test: expired/revoked tokens are rejected
- [ ] Deprecate the ucan-auth openspec (superseded by this integration)

## Phase 5: Blob storage

- [ ] Implement `AspenBlobStore` wrapping aspen's iroh-blobs
- [ ] Implement `LocalBlobStore` for standalone mode (content-addressed dir)
- [ ] Wire blob store into Matrix file attachment handler
- [ ] Wire blob store into session export/import
- [ ] Wire blob store into router response cache (large entries)
- [ ] Implement blob protection tags for GC safety
- [ ] Test: store blob on Node A, fetch from Node B via P2P
- [ ] Test: session delete unprotects blobs, GC collects orphans

## Phase 6: Distributed agents

- [ ] Implement `ClankerAgentWorker` as aspen job worker
- [ ] Implement `AgentPromptPayload` / `AgentPromptResult` serde types
- [ ] Wire `delegate` tool to submit aspen jobs in cluster mode
- [ ] Implement job progress streaming back to parent agent
- [ ] Implement distributed session locks via aspen coordination
- [ ] Implement per-user rate limiting via aspen coordination
- [ ] Implement cluster-wide agent semaphore (bounded concurrency)
- [ ] Implement `clankers status --cluster` showing all active agents/jobs
- [ ] Test: subagent job runs on different node than parent
- [ ] Test: node failure mid-job triggers retry on another node
- [ ] Test: session lock prevents concurrent prompts across nodes

## Phase 7: Plugin migration

- [ ] Implement `clankers-plugin-sdk` guest crate (Hyperlight-based)
- [ ] Port `clankers-hash` plugin from Extism to Hyperlight
- [ ] Port `clankers-text-stats` plugin from Extism to Hyperlight
- [ ] Implement dual runtime (Extism + Hyperlight) for transition period
- [ ] Implement plugin manifest storage in aspen KV
- [ ] Implement plugin WASM binary storage in iroh-blobs
- [ ] Implement `clankers plugin migrate` CLI tool
- [ ] Implement host function permission enforcement
- [ ] Implement KV namespace isolation for plugins
- [ ] Implement hot reload via ArcSwap
- [ ] Test: existing Extism plugins still work (backwards compat)
- [ ] Test: new Hyperlight plugin with KV access
- [ ] Remove Extism dependency (after all plugins migrated)

## Phase 8: Polish

- [ ] Router state in aspen KV (provider registry, circuit breaker, cache metadata)
- [ ] Add `[cluster]` section to default config template
- [ ] Update README with cluster mode documentation
- [ ] Update `clankers --help` with cluster flags
- [ ] Add `clankers cluster status` command
- [ ] Performance benchmark: standalone vs. cluster latency for prompt execution
- [ ] Document aspen cluster setup for clankers users
