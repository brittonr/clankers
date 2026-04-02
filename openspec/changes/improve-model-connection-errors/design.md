## Context

Errors from model connections traverse four crate boundaries: `clanker-router::Error` → `clankers-provider::ProviderError` → `clankers-agent::AgentError` → `src::Error`. Each boundary currently discards structured information (HTTP status codes, retryability classification) by converting to string messages. The clanker-router proxy binary has no OAuth refresh loop, so its tokens expire and produce confusing 429 errors.

The retry/fallback infrastructure already exists at two levels:
- **HTTP level**: `send_streaming` / `do_request_with_retry` retries 3 times with exponential backoff on retryable status codes
- **Router level**: `RouterProvider::complete` tries fallback model chains and skips providers in rate-limit cooldown

What's missing is a third level (turn-level retry in the agent loop) and the structured context to drive it.

## Goals / Non-Goals

**Goals:**
- HTTP status and retryability survive the full error chain so each layer can make informed decisions
- The agent loop retries retryable failures that exhaust the HTTP+router layers
- Users see actionable error messages identifying which providers failed and why
- The clanker-router proxy keeps its OAuth tokens fresh without external coordination
- Retry logging goes through `tracing`, not raw stderr

**Non-Goals:**
- Changing the retry/backoff parameters (current 3 retries with jitter is fine)
- Adding new provider backends
- Sharing auth stores between clankers and clanker-router (they have different store formats; the router should self-refresh)
- Partial content recovery from mid-stream failures (valuable but separate change)

## Decisions

### 1. Enrich `AgentError` with status and retryability

Add `status: Option<u16>` and `retryable: bool` fields to `AgentError::ProviderStreaming`. This is the minimal change that lets the turn loop and controller make retry/display decisions without parsing message strings.

**Alternative**: Embed the full `ProviderError` inside `AgentError`. Rejected because it couples `clankers-agent` to `clankers-provider` error internals and the agent crate only needs two bits of information.

### 2. Turn-level retry with capped attempts

Wrap the `execute_turn` call in `run_turn_loop` with a retry loop: on retryable errors, backoff and retry the same turn up to 2 additional times. The turn's message state is not modified on failure (the failed attempt produced no assistant message), so retry is safe.

**Alternative**: Retry at the controller level (in `handle_prompt`). Rejected because the controller doesn't have access to the per-turn context and retrying the entire prompt is wasteful when only the last turn failed.

### 3. Per-attempt error collection for exhaustion summary

Change `RouterProvider::complete` to collect `(provider_name, model_id, status, message)` tuples as providers fail. When all are exhausted, build a multi-line summary. The last error's status code is preserved for retryability classification upstream.

### 4. OAuth refresh loop in clanker-router binary

Add a `CredentialRefresher` to the router binary (not the library crate) that:
- Reads the router's own `~/.config/clanker-router/auth.json`
- Runs `clanker_router::oauth::refresh_token()` proactively before expiry (same 5-minute-before-expiry strategy as `clankers-provider::CredentialManager`)
- Writes back with file locking (same `fs4` approach)
- Updates the in-memory `AnthropicProvider` credential via `update_credential()`

This lives in the binary crate (`src/bin/clanker_router/`) not the library, because the library backends are stateless providers — the refresh lifecycle is an application concern.

**Alternative**: Have the router read from pi's `~/.pi/agent/auth.json`. Rejected because it couples the router to pi's auth format and assumes pi is running.

### 5. Fix lossy error conversions

- `RouterCompatAdapter::complete`: use `ProviderError::from(e)` instead of `provider_err(e.to_string())`
- `RpcProvider::complete`: same fix
- `From<clanker_router::Error> for ProviderError`: already correct, just ensure it's used

### 6. Replace `eprintln!` with `tracing::warn!`

Two call sites in `anthropic/api.rs` use `eprintln!` for retry logging. Replace with `tracing::warn!`. The tracing subscriber in the binary crate routes these to the log file, not the TUI.

### 7. Warn on SSE parse failures

In `anthropic/streaming.rs`, the `.ok()?` pattern silently drops malformed events. Replace with `tracing::warn!` on parse failure, continue processing. If 5+ consecutive events fail to parse, emit a `StreamEvent::Error` to surface the problem.

## Risks / Trade-offs

**[Turn-level retry adds latency on hard failures]** → Mitigated by the retryability check: non-retryable errors (400, 401, 403) skip the retry loop entirely. Only 429/500/502/503/529 trigger retries, and the backoff is short (1-4s).

**[OAuth refresh in the router binary adds a background task]** → Same pattern already proven in `clankers-provider::CredentialManager`. Uses `Weak` reference so the task exits when the provider is dropped.

**[Enriching `AgentError` is a breaking change for downstream matchers]** → `AgentError` is internal to the workspace. The new fields have defaults (`status: None`, `retryable: false`) so existing `From` impls only need minor updates.

**[Per-attempt error collection allocates on every fallback attempt]** → Bounded by the number of fallback models (typically 2-3). Negligible cost compared to the HTTP round-trips.
