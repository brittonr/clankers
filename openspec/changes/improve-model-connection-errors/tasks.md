## 1. Structured Error Propagation

- [x] 1.1 Add `status: Option<u16>` and `retryable: bool` fields to `AgentError::ProviderStreaming` in `crates/clankers-agent/src/error.rs`
- [x] 1.2 Update `From<ProviderError> for AgentError` to carry status and retryability from `ProviderError`
- [x] 1.3 Update `From<clanker_router::Error> for AgentError` to carry status from router errors
- [x] 1.4 Update `From<AgentError> for Error` in `src/error.rs` to propagate the new fields into the binary crate's error type
- [x] 1.5 Fix `RouterCompatAdapter::complete` in `crates/clankers-provider/src/router.rs` to use `ProviderError::from(e)` instead of `provider_err(e.to_string())`
- [x] 1.6 Fix `RpcProvider::complete` in `crates/clankers-provider/src/rpc_provider.rs` to use structured error conversion
- [x] 1.7 Replace `eprintln!` calls in `crates/clankers-provider/src/anthropic/api.rs` with `tracing::warn!`
- [x] 1.8 Add `tracing::warn!` on SSE parse failures in `crates/clankers-provider/src/anthropic/streaming.rs` with consecutive-failure tracking

## 2. Turn-Level Retry

- [x] 2.1 Add a retry wrapper around the `execute_turn` call in `run_turn_loop` (`crates/clankers-agent/src/turn/mod.rs`) that retries up to 2 times on retryable errors with backoff
- [x] 2.2 Check cancellation token between retry attempts; return `Cancelled` if set during backoff
- [x] 2.3 Verify that failed turn attempts do not append messages to the conversation history
- [x] 2.4 Add tests: retryable error recovered on second attempt, non-retryable error skips retry, cancellation during backoff

## 3. Provider Failure Summary

- [x] 3.1 Add a `ProviderAttempt` struct (provider name, model ID, status, message) and collect attempts in `RouterProvider::complete`
- [x] 3.2 Build a multi-line summary message when all providers are exhausted, including cooldown-skipped providers
- [x] 3.3 Preserve the last attempt's HTTP status code on the returned `ProviderError`
- [x] 3.4 Add tests: two-provider failure summary, single-provider failure, cooldown-skipped provider in summary

## 4. Router OAuth Refresh

- [x] 4.1 Add a `CredentialRefresher` struct in `clanker-router/src/bin/clanker_router/` that reads the router's auth store, refreshes proactively before expiry, and writes back with file locking
- [x] 4.2 Wire the refresher into the router binary's startup so it runs as a background task alongside the proxy
- [ ] 4.3 Update the Anthropic backend's `do_request_with_retry` to trigger a reactive refresh on 401 when a refresher is available
- [x] 4.4 Use a `CancellationToken` in the refresh loop so it exits on proxy shutdown
- [x] 4.5 Add tests: proactive refresh before expiry, reactive refresh on 401, refresh failure logged without crash, concurrent refresh file locking

## 5. Verification

- [x] 5.1 Run `cargo nextest run` — all existing tests pass
- [x] 5.2 Run `cargo clippy -- -D warnings` — no new warnings
- [ ] 5.3 Manual test: start router proxy with expired OAuth token, send a request, confirm refresh fires and request succeeds
