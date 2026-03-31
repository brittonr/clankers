## Why

clanker-router only serves OpenAI-compatible endpoints (`/v1/chat/completions`, `/v1/models`). When clankers sets `ANTHROPIC_BASE_URL` to the colocated router for load balancing, its Anthropic provider hits `$BASE_URL/v1/messages` with native Anthropic format and gets HTTP 404. There is no way to use the router's credential pool, rate-limit tracking, and fallback chains from clankers without losing Anthropic-specific features (prompt caching breakpoints, thinking signatures, cache token reporting) that don't survive OpenAI format translation.

## What Changes

- Add a native Anthropic Messages API endpoint (`POST /v1/messages`) to clanker-router's HTTP proxy, alongside the existing OpenAI endpoint.
- The new endpoint accepts Anthropic-format requests, routes them through the existing `Router.complete()` pipeline (credential rotation, rate limits, fallbacks, usage recording), and streams back native Anthropic SSE events.
- clankers' Anthropic provider works unmodified — `ANTHROPIC_BASE_URL=http://127.0.0.1:4000` just works.
- The existing `/v1/chat/completions` endpoint is unchanged. Both inbound formats share the same routing core.

## Capabilities

### New Capabilities
- `anthropic-messages-endpoint`: Native Anthropic `/v1/messages` inbound endpoint on the proxy — request parsing, SSE response encoding, and integration with the router's credential/rate-limit/fallback pipeline.

### Modified Capabilities

## Impact

- **clanker-router**: `src/proxy/mod.rs` gets a new handler and route. New module or section for Anthropic request/response conversion (inverse of the existing `ChunkConverter`). Axum router gains `POST /v1/messages`.
- **clankers**: No code changes. Setting `ANTHROPIC_BASE_URL` to the router address becomes a supported configuration.
- **Deployment**: NixOS service configs that set `ANTHROPIC_BASE_URL` to the router will work without additional env vars or format bridging.
