## ADDED Requirements

### Requirement: Proxy accepts native Anthropic Messages API requests
The proxy SHALL accept `POST /v1/messages` requests with a JSON body conforming to the Anthropic Messages API format. The request body MUST include `model`, `messages`, and `max_tokens` fields. The `stream` field MUST be `true`; the proxy SHALL reject non-streaming requests with HTTP 400.

#### Scenario: Valid streaming request
- **WHEN** a client sends `POST /v1/messages` with `{"model":"claude-sonnet-4-5-20250514","messages":[{"role":"user","content":"Hello"}],"max_tokens":1024,"stream":true}`
- **THEN** the proxy returns HTTP 200 with `Content-Type: text/event-stream` and streams Anthropic-format SSE events

#### Scenario: Non-streaming request rejected
- **WHEN** a client sends `POST /v1/messages` with `"stream": false` or `stream` omitted
- **THEN** the proxy returns HTTP 400 with an Anthropic-format error body: `{"type":"error","error":{"type":"invalid_request_error","message":"..."}}`

#### Scenario: Missing required fields
- **WHEN** a client sends `POST /v1/messages` without `model` or `messages`
- **THEN** the proxy returns HTTP 400 with an Anthropic-format error body

### Requirement: Requests route through the standard router pipeline
The proxy SHALL convert incoming Anthropic requests to `CompletionRequest` and dispatch through `Router.complete()`. All router features — credential pool rotation, rate-limit tracking, fallback chains, usage recording, response caching — SHALL apply to Anthropic-format requests identically to OpenAI-format requests.

#### Scenario: Credential rotation on rate limit
- **WHEN** a client sends a request through `/v1/messages` and the primary credential returns HTTP 429
- **THEN** the router rotates to the next healthy credential in the pool and retries, identical to the behavior for `/v1/chat/completions` requests

#### Scenario: Usage recorded
- **WHEN** a request through `/v1/messages` completes successfully
- **THEN** the router records input tokens, output tokens, cache creation tokens, cache read tokens, and estimated cost in the usage database

#### Scenario: Fallback on provider failure
- **WHEN** a request through `/v1/messages` targets a model whose provider is in rate-limit cooldown
- **THEN** the router falls back to the next model in the fallback chain, as configured in `FallbackConfig`

### Requirement: Response streams native Anthropic SSE events
The proxy SHALL convert `StreamEvent` values from the router into Anthropic-format SSE events. Each SSE event SHALL have an `event:` field and a `data:` field containing JSON matching the Anthropic Messages API streaming format.

#### Scenario: Text completion stream
- **WHEN** the router produces `MessageStart`, `ContentBlockStart(Text)`, `ContentBlockDelta(TextDelta)`, `ContentBlockStop`, `MessageDelta`, `MessageStop`
- **THEN** the proxy emits SSE events: `message_start`, `content_block_start`, `content_block_delta`, `content_block_stop`, `message_delta`, `message_stop` with JSON payloads matching Anthropic's documented format

#### Scenario: Tool use stream
- **WHEN** the router produces `ContentBlockStart(ToolUse)` followed by `ContentBlockDelta(InputJsonDelta)` events
- **THEN** the proxy emits `content_block_start` with `{"type":"tool_use","id":"...","name":"..."}` and `content_block_delta` with `{"type":"input_json_delta","partial_json":"..."}`

#### Scenario: Extended thinking stream
- **WHEN** the router produces `ContentBlockStart(Thinking)` followed by `ContentBlockDelta(ThinkingDelta)` and `ContentBlockDelta(SignatureDelta)` events
- **THEN** the proxy emits `content_block_start` with `{"type":"thinking"}`, `content_block_delta` with `{"type":"thinking_delta"}`, and `content_block_delta` with `{"type":"signature_delta"}`

#### Scenario: Error during streaming
- **WHEN** the router produces an `Error { error }` event
- **THEN** the proxy emits `event: error` with `data: {"type":"error","error":{"type":"server_error","message":"..."}}`

#### Scenario: Usage includes cache tokens
- **WHEN** the router produces `MessageDelta` with `usage.cache_creation_input_tokens > 0` or `usage.cache_read_input_tokens > 0`
- **THEN** the proxy includes `cache_creation_input_tokens` and `cache_read_input_tokens` in the `message_delta` usage JSON

### Requirement: System prompt cache_control preserved
The proxy SHALL preserve `cache_control` annotations on system prompt blocks through the request pipeline. When the incoming request contains a `system` array with blocks that include `cache_control`, the Anthropic backend's outbound request to the upstream API SHALL include those same `cache_control` annotations.

#### Scenario: System blocks with cache_control
- **WHEN** a client sends `"system": [{"type":"text","text":"You are...","cache_control":{"type":"ephemeral"}}]`
- **THEN** the outbound request to Anthropic's API includes the same system blocks with `cache_control` intact

#### Scenario: System blocks without cache_control
- **WHEN** a client sends `"system": [{"type":"text","text":"You are..."}]` with no `cache_control`
- **THEN** the Anthropic backend applies its default caching behavior (adding `cache_control` per its own logic)

### Requirement: Proxy authentication applies
The proxy's existing bearer token authentication SHALL apply to `/v1/messages`. The client's `x-api-key` header SHALL NOT be forwarded to the upstream Anthropic API — the router uses its own credential pool.

#### Scenario: Valid proxy auth
- **WHEN** `allowed_keys` is configured and the client sends `Authorization: Bearer <valid-key>` to `/v1/messages`
- **THEN** the request is accepted and processed

#### Scenario: Missing proxy auth
- **WHEN** `allowed_keys` is configured and the client sends no `Authorization` header to `/v1/messages`
- **THEN** the proxy returns HTTP 401 with an Anthropic-format error body

#### Scenario: Invalid proxy auth
- **WHEN** `allowed_keys` is configured and the client sends an invalid bearer token to `/v1/messages`
- **THEN** the proxy returns HTTP 403 with an Anthropic-format error body

#### Scenario: No auth configured
- **WHEN** `allowed_keys` is empty (no auth required)
- **THEN** requests to `/v1/messages` are accepted without an `Authorization` header

### Requirement: Anthropic-format error responses
Error responses from `/v1/messages` SHALL use Anthropic's error format: `{"type":"error","error":{"type":"<error_type>","message":"<description>"}}`. This applies to proxy-level errors (auth, bad request) and upstream errors forwarded from the router.

#### Scenario: Validation error format
- **WHEN** a client sends an invalid request to `/v1/messages`
- **THEN** the response body is `{"type":"error","error":{"type":"invalid_request_error","message":"..."}}`

#### Scenario: Auth error format
- **WHEN** a client sends a request with invalid credentials to `/v1/messages`
- **THEN** the response body is `{"type":"error","error":{"type":"authentication_error","message":"..."}}`

#### Scenario: Upstream error format
- **WHEN** the upstream Anthropic API returns an error and all retries/fallbacks are exhausted
- **THEN** the response body is `{"type":"error","error":{"type":"api_error","message":"..."}}`
