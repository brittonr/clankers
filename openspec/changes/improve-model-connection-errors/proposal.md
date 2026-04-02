## Why

When a model connection fails, structured error context (HTTP status codes, retryability flags, per-provider failure details) is discarded as errors propagate through the `AgentError` → `ProviderError` → `Error` chain. Users see opaque messages like "All providers exhausted" with no way to distinguish rate limiting from auth failure from network unreachability. Retry logging bypasses `tracing` and writes directly to stderr, corrupting the TUI in daemon mode.

## What Changes

- Carry HTTP status codes and retryability through `AgentError` so the turn loop and controller can make informed retry/display decisions
- Replace `eprintln!` retry logging in the Anthropic HTTP client with `tracing::warn!`
- Fix `RouterCompatAdapter` and `RpcProvider` error conversions to preserve structured status codes instead of flattening to strings
- Build per-provider failure summaries when all fallbacks are exhausted ("anthropic:sonnet → 429 rate limited, openai:gpt-4o → 529 overloaded")
- Add turn-level retry with backoff in the agent loop for retryable errors that survive the HTTP/router retry layers
- Add OAuth token refresh loop to the clanker-router proxy so it doesn't go stale when running as a standalone proxy (currently only clankers-provider's `CredentialManager` refreshes tokens; the router binary's auth store rots)
- Log SSE parse failures instead of silently dropping malformed events

## Capabilities

### New Capabilities

- `structured-error-propagation`: Preserve HTTP status, retryability, and provider identity through the full error chain from HTTP client to user-facing display
- `turn-level-retry`: Agent loop retries retryable provider failures with backoff before surfacing the error to the user
- `provider-failure-summary`: When all providers/fallbacks fail, build a human-readable summary of what was tried and why each failed
- `router-oauth-refresh`: The clanker-router proxy gets its own OAuth token refresh loop so credentials don't expire when running standalone

### Modified Capabilities

## Impact

- `crates/clankers-agent/src/error.rs` — `AgentError::ProviderStreaming` gains status/retryable fields
- `crates/clankers-agent/src/turn/execution.rs` — Turn-level retry logic wrapping `execute_turn`
- `crates/clankers-provider/src/error.rs` — No structural changes, but conversions used more carefully
- `crates/clankers-provider/src/router.rs` — `RouterCompatAdapter` error conversion fix, exhaustion summary
- `crates/clankers-provider/src/rpc_provider.rs` — Structured error conversion
- `crates/clankers-provider/src/anthropic/api.rs` — `eprintln!` → `tracing::warn!`
- `crates/clankers-provider/src/anthropic/streaming.rs` — Warn on SSE parse failures
- `src/error.rs` — `From<AgentError>` updated for new fields
- `src/modes/event_loop_runner/mod.rs` — Error display may improve from richer messages
- `clanker-router` (external) — OAuth refresh loop in the proxy/daemon binary, reading from its own `auth.json`
