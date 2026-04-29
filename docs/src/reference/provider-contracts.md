# Provider Contracts

This page indexes the request/stream contracts that keep `clankers-provider`, `clanker-router`, and agent request construction aligned.

## Request shape ownership

- `clankers-agent` converts engine-native `EngineModelRequest` values into `clankers_provider::CompletionRequest` in `crates/clankers-agent/src/turn/execution.rs`.
- `clankers-provider` owns the local provider trait and provider compatibility adapters.
- `clanker-router` owns routed backend implementations, failover, caching, OAuth-aware backends, and RPC/proxy request handling.
- Routed provider calls must preserve provider-native message content. Do not rebuild routed request messages with lossy `serde_json::to_value(...)` conversions of `AgentMessage`.

## Required request fields

`CompletionRequest` must stay aligned across `clankers-provider` and `clanker-router` for shared fields:

- `model`
- `messages`
- `system_prompt`
- `max_tokens`
- `temperature`
- `tools`
- `thinking`
- `no_cache`
- `cache_ttl`
- `extra_params`

`extra_params` is part of the request contract. It carries cross-cutting metadata that provider backends may need without changing every backend signature. The most important current key is `_session_id`; losing it breaks session-scoped provider metadata and routed request correlation.

## Existing deterministic rails

When touching request construction or provider/router shared fields, keep these tests current:

- `crates/clankers-provider/src/lib.rs`
  - constructor inventory test requiring every `CompletionRequest { ... }` constructor in selected provider files to set `extra_params`,
  - shared-field serde projection parity between `clankers_provider::CompletionRequest` and `clanker_router::CompletionRequest`,
  - empty `extra_params` omission parity.
- `crates/clankers-provider/src/rpc_provider.rs`
  - RPC request conversion tests that preserve `_session_id` and provider-native message JSON.
- `crates/clankers-provider/src/router.rs`
  - routed adapter tests and request conversion checks.
- `crates/clanker-router/src/backends/openai_codex.rs`
  - OpenAI Codex request fixture and entitlement/streaming contract tests.

If you add a real `CompletionRequest` constructor, update the constructor-count inventory instead of bypassing it.

## Stream/SSE contract

Provider stream claims need runtime parser coverage, not only helper-level state-machine tests.

- Anthropic stream parsing enters through `crates/clankers-provider/src/anthropic/streaming.rs`.
- OpenAI Codex stream normalization enters through the routed backend in `crates/clanker-router/src/backends/openai_codex.rs`.
- Tests that claim SSE normalization should feed raw `text/event-stream` bytes through the real parser entrypoint, then assert normalized stream events.

## Change checklist

Before merging a provider request/stream change:

1. Did `_session_id` survive from `Agent.session_id` through `CompletionRequest.extra_params`?
2. Did provider and router shared-field serde projections still match?
3. Did every real `CompletionRequest { ... }` constructor set `extra_params`?
4. Did routed backends receive provider-native message JSON rather than empty/lossy converted input?
5. If stream behavior changed, is there a runtime parser-entrypoint test using raw SSE bytes?
6. If auth or provider prefix behavior changed, does explicit known-but-unavailable provider routing still fail closed?
