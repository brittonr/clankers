## Why

Clankers has credential pool failover for rate-limited Anthropic accounts, but API error handling elsewhere is ad-hoc string matching scattered across provider code. Different error types (auth failure, billing exhaustion, context overflow, model not found, server overload) need different recovery strategies (retry with backoff, rotate credential, compress context, fallback to another model, abort). Hermes has a structured error taxonomy with `ClassifiedError` that carries recovery hints the retry loop can act on. Without this, clankers either over-retries unrecoverable errors or gives up too early on transient ones.

## What Changes

- Add a structured error classifier in `clankers-provider` with a `FailoverReason` taxonomy and `ClassifiedError` type
- Pattern-match API error responses (status codes, error messages) into the taxonomy across all supported providers (Anthropic, OpenAI, OpenRouter, Codex, local endpoints)
- Attach recovery hints to classified errors: retryable, should_compress, should_rotate, should_fallback
- Wire the classifier into the existing retry/failover paths in the provider and router layers

## Capabilities

### New Capabilities
- `error-classification`: Structured API error taxonomy with provider-specific pattern matching and recovery-hint annotations, replacing scattered ad-hoc error string matching.

### Modified Capabilities

## Impact

- `crates/clankers-provider/` — new `error_classifier` module; modify existing error handling in `anthropic/`, `openai_codex.rs`, and router integration
- `clanker-router` (external) — may need `ClassifiedError` in its error types for the routing layer to make informed fallback decisions
- `crates/clankers-agent/src/turn.rs` — retry loop consults classifier instead of raw error strings
- `crates/clankers-provider/src/anthropic/mod.rs` — credential pool rotation driven by `should_rotate` instead of raw 429 matching
