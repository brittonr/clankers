## Context

clanker-router's HTTP proxy currently exposes only OpenAI-compatible endpoints (`/v1/chat/completions`, `/v1/models`). Internally it translates OpenAI-format requests into `CompletionRequest`, routes them through `Router.complete()` (credential rotation, rate limits, fallbacks, usage recording), and the Anthropic backend converts back to native format for the upstream API.

clankers' Anthropic provider speaks native Anthropic format (`POST /v1/messages` with SSE streaming). When `ANTHROPIC_BASE_URL` points at the router, the provider hits `/v1/messages` and gets 404. The only current workaround is to bypass the router entirely, losing its load-balancing features.

The router's internal `StreamEvent` enum is already isomorphic to Anthropic's SSE event types (`message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop`). The `CompletionRequest` carries messages as `Vec<serde_json::Value>`, so Anthropic-format message JSON passes through without structural loss.

## Goals / Non-Goals

**Goals:**
- Add `POST /v1/messages` to clanker-router's proxy that accepts native Anthropic Messages API requests and returns native Anthropic SSE responses.
- Route through the same `Router.complete()` pipeline — credential pool rotation, rate-limit tracking, fallback chains, usage recording, response caching all apply.
- Preserve Anthropic-specific features end-to-end: prompt caching breakpoints (`cache_control`), extended thinking with signatures, cache token reporting (`cache_creation_input_tokens`, `cache_read_input_tokens`).
- Zero changes to clankers — `ANTHROPIC_BASE_URL=http://127.0.0.1:4000` works as-is.

**Non-Goals:**
- Translating OpenAI-format requests to Anthropic format (the existing `/v1/chat/completions` → Anthropic backend path already handles this).
- Supporting non-streaming Anthropic requests (clankers always streams; non-streaming can be added later).
- Forwarding Anthropic beta headers from the client — the router's Anthropic backend sets its own headers based on the credential type.
- Authentication against Anthropic from the proxy request's `x-api-key` header — the router uses its own credential pool, not the client's API key.

## Decisions

### 1. Inbound request conversion: Anthropic JSON → CompletionRequest

The handler deserializes the incoming Anthropic request body, extracts fields into `CompletionRequest`:

- `model` → `model`
- `system` (array of blocks) → join text blocks into `system_prompt`. Preserve the raw JSON in `extra_params["_anthropic_system"]` so the Anthropic backend can reconstruct cache_control breakpoints on the outbound request.
- `messages` → `messages` (pass through as `Vec<Value>` — already the right format for the Anthropic backend)
- `max_tokens` → `max_tokens`
- `temperature` → `temperature`
- `tools` → `tools` (convert from Anthropic tool format to `ToolDefinition`)
- `thinking` → `thinking`
- `stream` — must be `true` (reject non-streaming with 400)

**Alternative considered:** Bypassing `CompletionRequest` and forwarding the raw request body directly to the Anthropic backend. Rejected because it skips the router's credential rotation, rate-limit, fallback, and caching pipeline.

### 2. Outbound response conversion: StreamEvent → Anthropic SSE

Write an `AnthropicSseConverter` (inverse of the existing `ChunkConverter` for OpenAI). Maps:

| StreamEvent | Anthropic SSE event |
|---|---|
| `MessageStart { message }` | `event: message_start` with `{"type":"message_start","message":{...}}` |
| `ContentBlockStart { index, content_block }` | `event: content_block_start` |
| `ContentBlockDelta { index, delta }` | `event: content_block_delta` |
| `ContentBlockStop { index }` | `event: content_block_stop` |
| `MessageDelta { stop_reason, usage }` | `event: message_delta` |
| `MessageStop` | `event: message_stop` |
| `Error { error }` | `event: error` |

The mapping is nearly 1:1 because `StreamEvent` was modeled after Anthropic's protocol. The converter reconstructs the JSON payloads with the exact field names and structure that Anthropic clients expect.

Cache token fields (`cache_creation_input_tokens`, `cache_read_input_tokens`) are included in `message_delta` usage — they're already in the `Usage` struct.

### 3. Authentication: proxy-level, not passthrough

The existing proxy auth (`Authorization: Bearer <key>` checked against `allowed_keys`) applies to the new endpoint. The client's `x-api-key` or `Authorization` header is NOT forwarded to Anthropic — the router uses its own credential pool. This is the same model as the OpenAI endpoint.

### 4. System prompt round-trip with cache_control preservation

Anthropic's system prompt is an array of blocks, each potentially with `cache_control`. The simple `system_prompt: Option<String>` field in `CompletionRequest` loses this structure. Two options:

**Chosen:** Store the raw Anthropic system blocks in `extra_params["_anthropic_system"]` as JSON. The Anthropic backend checks for this key and uses the pre-built blocks instead of constructing its own from `system_prompt`. This is a targeted change to the Anthropic backend's `build_request_body`.

**Alternative:** Make `system_prompt` an enum (`String | Vec<Value>`). Rejected — too invasive, touches every backend.

## Risks / Trade-offs

- **[Double cache_control tagging]** → The Anthropic backend adds its own `cache_control` breakpoints in `build_request_body`. When the client already placed them, this could double-tag. Mitigation: when `_anthropic_system` is present, skip the backend's own cache_control injection on system blocks. Messages already carry `cache_control` in the raw JSON; the backend only tags the last user message, which is fine.
- **[Anthropic API format drift]** → If Anthropic adds new SSE event types, the converter won't emit them. Mitigation: `StreamEvent` is the bottleneck — new event types need additions there regardless. The converter maps 1:1 from `StreamEvent`, so it stays in sync automatically.
- **[Non-streaming requests]** → Rejected with 400 for now. Some Anthropic clients might send `"stream": false`. Mitigation: document the limitation; add non-streaming support later if needed.
