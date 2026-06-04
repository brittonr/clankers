## 1. Anthropic request types and inbound parsing

- [x] 1.1 Add Anthropic request deserialization structs to `clanker-router/src/proxy/mod.rs` (or a new `anthropic_proxy` submodule): `AnthropicRequest`, `AnthropicMessage`, `AnthropicSystemBlock`, `AnthropicTool` — matching the Anthropic Messages API request schema
- [x] 1.2 Write `convert_anthropic_request(AnthropicRequest) -> CompletionRequest` — extract `model`, `max_tokens`, `temperature`, `tools`, `thinking` into `CompletionRequest` fields. Pass `messages` through as `Vec<Value>`. Store raw `system` blocks in `extra_params["_anthropic_system"]`
- [x] 1.3 Validate inbound request: reject `stream: false` or missing `stream` with HTTP 400 in Anthropic error format. Reject missing `model`/`messages`/`max_tokens` with 400

## 2. Anthropic SSE response converter

- [x] 2.1 Write `AnthropicSseConverter` that maps `StreamEvent` → Anthropic SSE JSON strings. One method per event type: `message_start`, `content_block_start` (text/thinking/tool_use), `content_block_delta` (text_delta/thinking_delta/input_json_delta/signature_delta), `content_block_stop`, `message_delta` (with cache token fields in usage), `message_stop`, `error`
- [x] 2.2 Write Anthropic-format error response helper: `anthropic_error_response(status, error_type, message) -> Response` producing `{"type":"error","error":{"type":"...","message":"..."}}`

## 3. Axum handler and routing

- [x] 3.1 Write `anthropic_messages` handler: deserialize body, check auth (reuse existing `check_auth` with Anthropic error format), convert request, call `Router.complete()`, stream response through `AnthropicSseConverter` as SSE
- [x] 3.2 Register `POST /v1/messages` route in `build_app()` alongside existing `/v1/chat/completions`

## 4. System prompt cache_control passthrough

- [x] 4.1 In the Anthropic backend's `build_request_body`, check for `extra_params["_anthropic_system"]`. When present, use the pre-built system blocks directly instead of constructing them from `system_prompt`. Skip the backend's own `cache_control` injection on system blocks in this case
- [x] 4.2 In the inbound converter, also extract a plain-text `system_prompt` (join text blocks) so non-Anthropic fallback backends still get a usable system prompt

## 5. Tests

- [x] 5.1 Unit test `convert_anthropic_request`: verify model, messages, system prompt, tools, thinking config, and `_anthropic_system` extra_param are set correctly
- [x] 5.2 Unit test `AnthropicSseConverter`: verify each `StreamEvent` variant produces correct Anthropic SSE JSON (text, tool_use, thinking with signature, error, usage with cache tokens)
- [x] 5.3 Integration test: send a well-formed `POST /v1/messages` to the proxy (backed by a mock provider) and verify the full SSE stream — `message_start` through `message_stop` with correct JSON structure
- [x] 5.4 Integration test: verify 400 on non-streaming request, 401/403 on bad auth (Anthropic error format), 400 on missing required fields
- [x] 5.5 Integration test: verify system blocks with `cache_control` survive the round-trip (inbound parse → `CompletionRequest` → Anthropic backend `build_request_body` output)
