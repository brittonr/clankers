# aspen-backend

## Intent

Clankers currently implements its own session storage (redb), P2P layer
(raw iroh), auth (flat allowlist), plugin system (Extism), and daemon
networking — all single-node, all bespoke.  Aspen already provides all of
these as battle-tested, distributed primitives: Raft-replicated KV, P2P blob
store, UCAN auth, Hyperlight WASM plugins, job queues, and coordination
primitives.

This change makes aspen the backend of clankers.  Instead of reinventing
infrastructure, clankers becomes a **stateless layer** over aspen's core
primitives — the same FoundationDB-inspired philosophy that aspen uses
internally.  A clankers node joins an aspen cluster and stores all its state
(sessions, config, usage, audit logs) in the replicated KV.  Blobs (file
attachments, screenshots, tool outputs) go to iroh-blobs.  Agent work is
dispatched through aspen's job queue.  Auth uses UCAN tokens from
aspen-auth.

The result: clankers sessions survive node failures, multiple clankers nodes
share state, and the entire infrastructure stack is one system instead of two.

## Scope

### In Scope

- Session storage on aspen KV (replicated conversations, turn history)
- Blob storage for file attachments and tool outputs (iroh-blobs via aspen)
- Shared iroh endpoint (aspen manages the QUIC endpoint, clankers registers ALPNs)
- Auth via aspen-auth UCAN tokens (replace flat allowlist + planned clankers-auth)
- Plugin migration from Extism to aspen's Hyperlight WASM host
- Agent job dispatch via aspen-jobs (subagent delegation as distributed jobs)
- Coordination primitives for multi-agent work (distributed locks, semaphores)
- Router state in aspen KV (provider credentials, circuit breaker state, cache)
- `clankers-aspen` bridge crate connecting the two systems
- Graceful degradation — clankers can still run standalone without an aspen cluster

### Out of Scope

- Forge / Git hosting features (aspen's forge is a separate concern)
- CI/CD pipeline integration (future change, after core integration)
- Secrets engine migration (clankers uses provider API keys, not Vault-style secrets)
- DNS integration (not relevant to a coding agent)
- Sharding (single clankers cluster won't need 256 shards)
- Federation (cross-org agent sharing is a separate concern)
- SQL layer (clankers doesn't query its own data with SQL)
- Changes to the TUI itself (TUI reads from the same abstractions)
- Matrix transport changes (bridge layer stays the same, storage changes beneath it)

## Approach

Introduce a `clankers-aspen` bridge crate that wraps aspen-client and
exposes clankers-specific storage traits.  The existing `SessionStore`,
`HistoryStore`, `UsageStore`, and `AuditStore` gain aspen-backed
implementations alongside the current redb ones.  At startup, clankers
either joins an aspen cluster (if configured) or falls back to local-only
redb — no breaking change for users who run a single node.

The migration is layered and incremental:

1. **Storage layer** — session/config/usage in aspen KV, blobs in iroh-blobs
2. **Networking** — share aspen's iroh endpoint, register clankers ALPNs
3. **Auth** — swap flat allowlist for aspen-auth UCAN verification
4. **Plugins** — migrate from Extism to Hyperlight WASM host
5. **Jobs** — subagent work dispatched through aspen-jobs across cluster

Each layer can ship independently.  Clankers always works standalone.
