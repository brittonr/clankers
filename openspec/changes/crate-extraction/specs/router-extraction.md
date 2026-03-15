# Router Crate Extraction (llm-router)

## Purpose

Extract `clankers-router` into a standalone multi-provider LLM routing
library with its own binary. At 16,750 lines with zero workspace deps,
its own CLI/TUI, proxy server, and iroh tunnel, this is already a
self-contained system that happens to live in the clankers tree.

This is the highest-value extraction — anyone building LLM tooling
needs provider routing, credential management, circuit breaking, and
retry logic.

## Requirements

### Crate identity

r[router.identity.name]
The extracted crate MUST be named `llm-router` (or chosen alternative).

r[router.identity.repo]
The crate MUST live in its own GitHub repository.

r[router.identity.binary]
The crate MUST produce a binary (renamed from `clankers-router`) that
provides the standalone proxy server and TUI.

r[router.identity.features]
The crate MUST preserve the existing feature flag structure:
- default: library only
- `proxy`: axum-based OpenAI-compatible proxy server
- `rpc`: iroh QUIC tunnel for remote proxy access
- `cli`: binary with clap CLI and ratatui TUI

### Source migration

r[router.source.modules]
The following module tree MUST be moved:

- `src/lib.rs` — crate root, module declarations
- `src/model.rs` — `Model` type
- `src/catalog.rs` — built-in model catalog
- `src/registry.rs` — model registry
- `src/provider.rs` — `ProviderBackend` trait, `ToolDefinition`, `CompletionRequest`
- `src/router/` — routing logic, fallback chains
- `src/backends/` — Anthropic, OpenAI-compat, HuggingFace backends
- `src/streaming.rs` — `StreamEvent`, SSE parsing
- `src/retry.rs` — exponential backoff with jitter
- `src/error.rs` — error types
- `src/auth.rs` — credential loading
- `src/oauth.rs` — OAuth PKCE flows
- `src/credential.rs` — credential types
- `src/credential_pool/` — multi-credential pooling
- `src/model_switch.rs` — runtime model switching
- `src/multi.rs` — multi-provider orchestration
- `src/quorum/` — quorum/consensus responses
- `src/db/` — redb storage (cache, rate limits, usage, request log)
- `src/proxy/` — OpenAI-compatible proxy server, iroh tunnel
- `src/rpc/` — RPC protocol, server, client, daemon integration
- `src/bin/clankers_router/` — binary entry point and TUI

r[router.source.no-clankers-refs]
The source MUST NOT reference "clankers" except in migration notes or
changelog. This includes:
- The `clankers/router/1` ALPN string MUST become `llm-router/1`
  (or a configurable value)
- Config paths under `~/.clankers/` MUST use `~/.llm-router/` or
  respect `XDG_CONFIG_HOME`
- Binary name MUST be `llm-router`, not `clankers-router`

### Provider trait

r[router.api.provider-trait]
The crate MUST export a `ProviderBackend` trait (or equivalent) that
provider implementations satisfy:

```rust
#[async_trait]
pub trait ProviderBackend: Send + Sync {
    fn name(&self) -> &str;
    fn supported_models(&self) -> &[&str];
    async fn complete(
        &self,
        request: &CompletionRequest,
        tx: mpsc::Sender<StreamEvent>,
    ) -> Result<()>;
}
```

r[router.api.model]
The crate MUST export `Model` with fields for: id, provider, context
window, max output, pricing, capabilities, aliases.

r[router.api.streaming]
The crate MUST export `StreamEvent` for incremental response delivery.

### Routing

r[router.routing.fallback]
The router MUST support configurable per-model fallback chains for
automatic failover when a provider is unavailable.

r[router.routing.circuit-breaker]
The router MUST implement per-provider/model circuit breaking with
Closed -> Open -> HalfOpen states, exponential backoff, and automatic
probe-on-cooldown-expiry.

r[router.routing.retry]
The router MUST implement retry with exponential backoff and full jitter.
It MUST respect `Retry-After` headers and detect retryable status codes
(429, 5xx).

### Credential management

r[router.credentials.multi-provider]
The crate MUST support credentials for: Anthropic (API key + OAuth),
OpenAI, Google/Gemini, DeepSeek, Groq, Mistral, OpenRouter, Together,
Fireworks, Perplexity, xAI, and arbitrary OpenAI-compatible endpoints.

r[router.credentials.oauth]
The crate MUST support OAuth PKCE flows with automatic token refresh,
file-locking for concurrent access, and proactive background refresh.

r[router.credentials.pool]
The crate MUST support credential pooling — multiple API keys per
provider with round-robin or least-recently-used selection.

### Proxy server

r[router.proxy.openai-compat]
The proxy MUST expose an OpenAI-compatible HTTP API (`/v1/chat/completions`,
`/v1/models`) for use with Cursor, aider, Continue, and other tools.

r[router.proxy.iroh-tunnel]
The proxy MUST support iroh QUIC tunneling for remote access without
port forwarding.

### Storage

r[router.storage.cache]
The crate MUST provide response caching with SHA-256 keys, TTL, LRU
eviction, and hit counting.

r[router.storage.rate-limits]
The crate MUST persist circuit breaker state across restarts.

r[router.storage.usage]
The crate MUST track per-model token usage and cost.

### Tests

r[router.tests.existing]
All existing tests (~661 lines in router/tests.rs plus others) MUST
pass in the extracted crate.

r[router.tests.ci]
The extracted repo MUST have CI running: cargo test, clippy, fmt check,
and nextest.

### Workspace migration

r[router.migration.re-export]
After extraction, `crates/clankers-router/` MUST become a thin wrapper:
```toml
[dependencies]
llm-router = { git = "https://github.com/brittonr/llm-router", features = ["rpc"] }
```
```rust
pub use llm_router::*;
```

r[router.migration.callers-unchanged]
All 26 files that import `clankers_router` MUST compile without changes.
Most go through `clankers-provider` which re-exports router types — this
transitive re-export chain MUST keep working.

r[router.migration.alpn-compat]
The ALPN migration from `clankers/router/1` to the new value MUST be
handled with a compatibility period where both ALPNs are accepted.

r[router.migration.workspace-builds]
`cargo check` and `cargo nextest run` MUST pass on the full workspace.
