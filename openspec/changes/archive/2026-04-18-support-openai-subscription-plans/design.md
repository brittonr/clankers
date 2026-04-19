## Context

OpenAI subscription support is blocked by three gaps:
1. Auth flows are Anthropic-specific in `src/commands/auth.rs` and `src/slash_commands/handlers/auth.rs`.
2. OpenAI discovery is API-key-only in `crates/clankers-provider/src/discovery.rs`.
3. Provider-specific request metadata is dropped before the router boundary because `clankers-provider::CompletionRequest` lacks `extra_params`.

This design freezes the private Codex wire contract inside OpenSpec so implementation does not depend on external TypeScript references.

## Goals / Non-Goals

**Goals**
- Authenticate ChatGPT Plus/Pro subscriptions as provider `openai-codex`.
- Preserve existing API-key `openai` behavior unchanged.
- Route Codex turns through a dedicated Responses backend with tools, thinking, retry, and usage normalization.
- Preserve a stable clankers session key into the backend for prompt caching / continuity.
- Make auth UX provider-aware in CLI and interactive mode.

**Non-Goals**
- Replacing API-key `openai` with Responses.
- Adding WebSocket transport in the first change.
- Supporting non-Codex consumer ChatGPT endpoints.
- Persisting derived `chatgpt_account_id` or entitlement state as new auth-store schema.

## Verification Plan

- Deterministic OAuth fixtures verify the authorize URL base, all required query parameters, the token endpoint, and required exchange/refresh form fields.
- Auth tests cover explicit `openai-codex` login, provider+account-scoped pending-login isolation, omitted-provider Anthropic defaults across login/status/switch/logout, grouped and provider-scoped CLI plus slash-command account/status output, separate OAuth validity/expiry reporting, proactive refresh, malformed-claim rejection, unsupported-plan suppression, unsupported-plan request blocking, entitlement-check-failed surfacing, and non-401 4xx handling.
- Auth tests also prove interactive `/login openai-codex` reloads provider credentials without restart and leaves existing `anthropic` plus API-key `openai` credentials unchanged.
- Deterministic constructor-inventory checks use the exact constructor-count inventory test in `crates/clankers-provider/src/lib.rs` over router-bound `CompletionRequest { ... }` sites covering `crates/clankers-agent/src/turn/execution.rs`, `src/modes/agent_task.rs`, `src/worktree/llm_resolver.rs`, `crates/clankers-provider/src/router.rs`, `crates/clankers-provider/src/rpc_provider.rs`, and router test/helper constructors.
- Deterministic cross-repo schema/serialization parity checks use the shared-field serde projection parity tests in `crates/clankers-provider/src/lib.rs` to prove `crates/clankers-provider::CompletionRequest` and `clanker-router::CompletionRequest` preserve `extra_params`, including `_session_id`, across local and RPC transport boundaries.
- Deterministic normal-request fixtures verify headers and body fields on initial, transient-retry, and 401 refresh-retry paths. Those checks use pinned literal JSON/header fixtures rather than expectations derived by calling the same request builders under test.
- Routed/local-RPC boundary verification includes a runtime seam in `crates/clankers-provider/src/router.rs` proving `RouterCompatAdapter` converts live conversation messages into provider-native `{role, content}` JSON before the router backend builds Codex `input`, so message shape survives the adapter boundary alongside `_session_id`.
- Deterministic entitlement-probe fixtures verify initial, transient-retry, and 401 refresh-retry probe paths, including required headers, included `accept: text/event-stream`, omitted `session_id`, JSON-path `error.code`, and the fixed probe body contract. Those checks use pinned literal JSON/header fixtures rather than expectations derived by calling the same request builders under test.
- Retry verification explicitly checks the fixed 1s/2s/4s no-jitter schedule, one-refresh maximum, and remaining-budget behavior after 401 refresh.
- Streaming verification includes helper-level mapping checks plus at least one real SSE seam test so `MessageStart`, `ContentBlockStart/Delta/Stop`, `MessageDelta`, `MessageStop`, reasoning signature replay, usage, and stop reasons are validated where raw SSE bytes enter the system.
- Discovery/model-resolution verification covers entitled, missing, unsupported, and stale-selection cases; exact current catalog contents (`gpt-5.3-codex`, `gpt-5.3-codex-spark`) and no extras; fail-closed explicit/resumed `openai-codex` requests via the `RouterProvider` fail-closed prefix sentinel; and unchanged API-key `openai` behavior. Routed discovery/completion tests run with daemon/cache isolation (`CLANKERS_NO_DAEMON=1`) so shared cooldown state cannot hide the real backend result.
- Docs verification includes an acceptance check or snapshot comparison proving CLI help, slash help, and provider docs cover `openai-codex` login, account naming, model selection, personal-use/plan limitations, unsupported-plan behavior, and unchanged API-key `openai` help paths.
- Finish-line checks include sibling `../clanker-router` pin update, `cargo nextest run`, `cargo clippy -- -D warnings`, `nix build .#clankers`, and one recorded live-credential smoke run against an already-authenticated real subscription account for status/account switching, model resolution, and one Codex turn.
- Auth/entitlement fixtures that need fake OAuth tokens use generated base64url JWT payloads with valid JSON claims rather than copied opaque literals so entitlement-path coverage does not silently degrade into JWT-parse failures.

## Decisions

### 1. Separate provider name

Use provider `openai-codex` for subscription-backed Codex access. Do not overload API-key `openai`.

### 2. Provider-aware OAuth drivers

Auth uses shared provider-aware drivers. For `openai-codex` the driver owns authorize URL generation, code exchange, refresh, and JWT claim extraction.

Frozen OAuth contract:
- authorize URL: `https://auth.openai.com/oauth/authorize`
- token URL: `https://auth.openai.com/oauth/token`
- authorize params: `response_type=code`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, `redirect_uri=http://localhost:1455/auth/callback`, `scope=openid profile email offline_access`, `code_challenge`, `code_challenge_method=S256`, `state`, `id_token_add_organizations=true`, `codex_cli_simplified_flow=true`, `originator=pi`
- code exchange form fields: `grant_type=authorization_code`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, `code`, `code_verifier`, `redirect_uri=http://localhost:1455/auth/callback`
- refresh form fields: `grant_type=refresh_token`, `client_id=app_EMoamEEZ73f0CkXaXp7hrann`, `refresh_token`
- account ID claim path: `payload["https://api.openai.com/auth"]["chatgpt_account_id"]`

Pending OAuth verifier/state is keyed by provider plus requested clankers account name from existing `--account <name>` / slash-account syntax. Derived `chatgpt_account_id` stays non-persisted. `openai-codex` participates in pre-expiry/background refresh; refreshed credentials update the persisted auth-store entry and reset in-memory entitlement state.

### 3. `_session_id` lives inside `extra_params`

`clankers-provider::CompletionRequest` gains `extra_params` to match `clanker-router::CompletionRequest`.
- `_session_id` is the literal persisted clankers session identifier stored inside `extra_params`
- `RouterCompatAdapter` and `RpcProvider` pass `extra_params` through unchanged
- interactive, agent-task, and resumed-session entry points reuse the same `_session_id` verbatim

### 4. Normal Codex request contract

All normal Codex turns use `POST https://chatgpt.com/backend-api/codex/responses`.

Required headers:
- `Authorization: Bearer <token>`
- `chatgpt-account-id: <id>`
- `OpenAI-Beta: responses=experimental`
- `originator: pi`
- `accept: text/event-stream`
- `content-type: application/json`
- `session_id: <_session_id>` only when a session key exists

Required body contract:
- `model`
- `store=false`
- `stream=true`
- `instructions` = clankers system prompt only
- `input` = prior conversation history
- `text={"verbosity":"medium"}` unless caller explicitly overrides verbosity
- `include=["reasoning.encrypted_content"]`
- `tool_choice="auto"`
- `parallel_tool_calls=true`
- `tools` only when tools are available
- `reasoning` only when the request enables thinking/reasoning
- `prompt_cache_key=<_session_id>` only when a session key exists
- normal requests retry only HTTP 429/500/502/503/504 with fixed 1s/2s/4s no-jitter backoff, use at most one 401 refresh + one immediate retry, continue with the remaining transient budget after refresh, and surface other non-401 4xx directly without retry

Input mapping:
- current turn content is appended as the final user item in `input`; on a first turn, `input` contains exactly that user item plus no prior history
- prior user turns → ordered user items in `input`
- prior assistant text/refusal turns → ordered assistant output items in `input`
- prior tool calls → `function_call` items with original call/item ID, tool name, and accumulated arguments
- prior tool results → `function_call_output` items paired to matching prior call ID
- replayed reasoning signatures → serialized reasoning items in `input`
- display-only thinking text is never resent as reasoning input

### 5. Reasoning replay

Store the serialized Codex reasoning item in `Content::Thinking.signature`. Replay that serialized payload on later `openai-codex` turns. Never replay display-only thinking text.

### 6. SSE first

First change supports SSE only. Preserve `_session_id` so later transport changes do not need another request-shape migration.

### 7. Provider-aware auth UX with Anthropic defaults preserved

Explicit provider selection becomes first-class:
- `clankers auth login --provider openai-codex`
- `/login openai-codex`
- `clankers auth switch --provider openai-codex <account>` and `/account switch openai-codex <account>`
- `clankers auth logout --provider openai-codex ...` and `/account logout openai-codex ...`
- `clankers auth status --all` and `/account --all`
- `clankers auth status --provider openai-codex` and slash-command equivalent provider-scoped account/status output
- CLI help, slash help, and provider docs describe the same `openai-codex` auth/account/model semantics

When provider is omitted, login/status/switch/logout keep Anthropic-compatible defaults.

### 8. Fixed Codex model catalog

Initial `openai-codex` catalog is exactly:
- `gpt-5.3-codex`
- `gpt-5.3-codex-spark`

`openai-codex/<model>` always resolves to the subscription backend. Plain `openai/<model>` stays on the API-key backend.
When discovery suppresses `openai-codex`, the resolver still reserves the known `openai-codex/...` prefix via a fail-closed sentinel in `RouterProvider` so explicit or resumed Codex selections surface the entitlement/provider error instead of silently falling back to Anthropic or API-key `openai`.

### 9. In-memory entitlement state

Each `openai-codex` account has in-memory entitlement state: `unknown`, `entitled`, or `not_entitled(reason, checked_at)`.

Entitlement probe contract:
- endpoint: `POST https://chatgpt.com/backend-api/codex/responses`
- headers: `Authorization`, `chatgpt-account-id`, `OpenAI-Beta: responses=experimental`, `originator=pi`, `accept: text/event-stream`, `content-type=application/json`
- omitted headers: `session_id`
- body: `model="gpt-5.3-codex"`, `store=false`, `stream=true`, `instructions="codex entitlement probe"`, `input=[{"role":"user","content":[{"type":"input_text","text":"ping"}]}]`, `text={"verbosity":"low"}`
- omitted body state: `tools`, `prompt_cache_key`, other session-specific state

Probe result contract:
- HTTP 2xx → `entitled`
- HTTP 403 or JSON path `error.code = "usage_not_included"` → `not_entitled`; either signal is authoritative even if the other is absent
- HTTP 401 → one refresh + one immediate retry
- HTTP 429/500/502/503/504 → fixed 1s/2s/4s retry policy
- other non-401 4xx → provider error, do not mutate entitlement state

State rules:
- successful login, provider reload, account switch, or token refresh resets state to `unknown`
- discovery, grouped/provider-scoped status, or the first explicit/resumed `openai-codex` request may trigger the probe when state is `unknown`
- `not_entitled` suppresses discovery, keeps the account authenticated, and surfaces “authenticated but not entitled for Codex use”
- probe exhaustion / non-entitlement provider errors keep state `unknown`, hide discovery, surface “authenticated, entitlement check failed”, and fail explicit/resumed requests closed with a retriable provider error
- explicit/resumed `openai-codex` turns must probe first when state is `unknown`; normal Codex requests are sent only after success
- entitlement state remains in memory only

### 10. SSE → generic stream boundary mapping

Raw Codex SSE events map to generic boundaries as follows:
- first emitted assistant item for a response → `MessageStart` exactly once
- `response.output_item.added` with `item.type="reasoning"` → `ContentBlockStart(Thinking)`
- `response.reasoning_summary_part.added` starts a summary segment inside the active thinking block without opening a second generic block
- `response.reasoning_summary_text.delta` → ordered thinking `ContentBlockDelta(...)`
- `response.reasoning_summary_part.done` closes that summary segment but does not emit `ContentBlockStop`
- `response.output_item.done` for reasoning → `ContentBlockStop` and capture serialized reasoning item into `Content::Thinking.signature`
- `response.content_part.added` for assistant output → `ContentBlockStart(Text)`
- `response.output_text.delta` / `response.refusal.delta` → ordered `ContentBlockDelta(TextDelta)`
- `response.output_item.done` for assistant message → `ContentBlockStop`
- `response.output_item.added` with `item.type="function_call"` → `ContentBlockStart(ToolUse)` using generic tool-call ID `<call_id>|<item.id>` and the raw tool name from the same event
- `response.function_call_arguments.delta` → ordered `ContentBlockDelta(InputJsonDelta)` for that same logical tool-call ID
- `response.function_call_arguments.done` seals final JSON payload for that same logical tool-call ID
- `response.output_item.done` for tool call → `ContentBlockStop`
- `response.completed` → `MessageDelta` carrying final usage + normalized stop reason, then `MessageStop`
- final status mapping: `completed` → normal stop; `incomplete` → incomplete/max-tokens-style stop; `failed` / `cancelled` → provider errors instead of successful `MessageStop`
- `queued` and `in_progress` are intermediate backend states only and do not surface as final generic stop reasons

## Human Review Gate

The change is not ready until one live ChatGPT Plus/Pro smoke run is executed after automated checks pass, using an already-authenticated real subscription account to verify live status/account switching, model resolution, and one Codex turn, and pass/fail evidence is recorded in the implementation PR or change notes. Fresh login and interactive reload remain covered by recorded-fixture and automated auth tests.

## Risks / Trade-offs

- Private endpoint or stream drift → freeze request + SSE contracts in-spec, guard with deterministic fixtures, keep one live smoke path.
- Session/replay bugs → keep `_session_id` and `Content::Thinking.signature` on explicit end-to-end tests through local, RPC, and resumed-session paths.
- Auth/entitlement UX complexity → keep Anthropic defaults when provider is omitted, expose provider-scoped status explicitly, and fail closed instead of silently falling back.
