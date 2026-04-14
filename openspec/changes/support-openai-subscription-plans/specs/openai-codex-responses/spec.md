## ADDED Requirements

### Requirement: OpenAI Codex is discovered as a separate provider family

The system SHALL expose Codex subscription models under provider `openai-codex` when entitled `openai-codex` OAuth credentials are available. API-key OpenAI models SHALL remain under provider `openai`.

#### Scenario: Subscription credentials expose Codex models

- GIVEN valid `openai-codex` OAuth credentials are configured
- WHEN provider discovery builds the router
- THEN the discovered provider list includes `openai-codex`
- AND Codex models resolve to that provider instead of the API-key `openai` backend

#### Scenario: User-visible catalog entries are provider-qualified

- GIVEN `openai-codex` credentials are configured
- WHEN the user lists or resolves available models
- THEN the initial Codex catalog exposes `gpt-5.1-codex`, `gpt-5.1-codex-max`, `gpt-5.1-codex-mini`, `gpt-5.2-codex`, `gpt-5.3-codex`, and `gpt-5.3-codex-spark` under provider `openai-codex`
- AND those identifiers stay distinct from API-key `openai` model identifiers so selection is unambiguous

#### Scenario: Missing subscription credentials hides the provider

- GIVEN no `openai-codex` credentials are configured
- WHEN provider discovery builds the router
- THEN `openai-codex` is not exposed as an available provider
- AND existing `openai` API-key behavior is unchanged

#### Scenario: Unsupported ChatGPT plan does not expose Codex models

- GIVEN OAuth login succeeded but the authenticated ChatGPT account does not have the supported Plus or Pro Codex entitlement
- WHEN provider discovery or entitlement refresh evaluates that account
- THEN `openai-codex` is suppressed from the available provider/model catalog for that account
- AND clankers surfaces that the account is authenticated but not entitled for Codex use

### Requirement: Unsupported or stale Codex selections fail closed

The system SHALL reject explicit or resumed `openai-codex` use for an authenticated account that lacks Codex entitlement, even if stale local session state or user selection still points at `openai-codex`.

#### Scenario: Explicit or resumed Codex request on an unsupported account

- GIVEN an account is authenticated for `openai-codex` but currently marked as not entitled for Codex use
- AND a resumed session or explicit provider/model selection targets `openai-codex`
- WHEN clankers prepares that turn
- THEN it surfaces a provider-auth or entitlement error before sending the normal Codex request
- AND explains that ChatGPT Plus or Pro is required for `openai-codex`
- AND does not fall back to API-key `openai` automatically

### Requirement: API-key OpenAI behavior stays unchanged

The system SHALL keep provider `openai` on its existing API-key transport and semantics. Supporting `openai-codex` SHALL NOT migrate API-key `openai` requests onto Codex Responses or OAuth-specific headers.

#### Scenario: API-key OpenAI request keeps current transport

- GIVEN the user selects provider `openai` with API-key credentials
- WHEN a request is sent through that provider
- THEN it uses the existing API-key OpenAI request path
- AND does not send Codex-specific OAuth headers or `openai-codex` session semantics

### Requirement: Stable session keys are reused across turns

The system SHALL derive `_session_id` directly from the clankers session identifier assigned to that conversation, preserve that value verbatim, and reuse the same value across later turns in that session so Codex prompt caching and session continuity stay stable.

#### Scenario: Session key is reused for later turns

- GIVEN a clankers session has an assigned session identifier
- WHEN multiple `openai-codex` turns are sent from that session
- THEN each request carries the same `_session_id` value
- AND a resumed session reuses that same value instead of generating a fresh per-request identifier

### Requirement: Provider-specific request metadata survives the clankers-to-router boundary

The system SHALL preserve provider-specific extra parameters, including stable internal session identifiers, when converting clankers requests into router requests.

#### Scenario: Session identifier survives adapter conversion

- GIVEN a clankers completion request includes `_session_id` in provider-specific extra parameters
- WHEN the request passes through `RouterCompatAdapter` or `RpcProvider`
- THEN the router-side completion request still contains the same `_session_id` value

### Requirement: Router-bound request constructors are audited as a complete set

The system SHALL treat every `CompletionRequest` construction path that can feed router execution as one audited set for provider-metadata preservation.

#### Scenario: Every router-bound request path preserves or initializes extras

- GIVEN any router-bound `CompletionRequest` construction path
- WHEN that request is built in `crates/clankers-agent/src/turn/execution.rs`, `src/modes/agent_task.rs`, `src/worktree/llm_resolver.rs`, `crates/clankers-provider/src/router.rs`, `crates/clankers-provider/src/rpc_provider.rs`, or router test/helper constructors
- THEN provider-specific `extra_params` are preserved or explicitly initialized at that construction site
- AND `_session_id` is not silently dropped before the router backend sees the request

### Requirement: Provider and router request schemas stay in parity for provider extras

The system SHALL keep `crates/clankers-provider::CompletionRequest` and `clanker-router::CompletionRequest` aligned for `extra_params`, including the `_session_id` key inside that map, across serde/local-RPC transport behavior so those fields survive the cross-repo boundary end to end.

#### Scenario: Provider request extras survive schema and serialization boundaries

- GIVEN a clankers provider-side completion request carries `extra_params` including `_session_id`
- WHEN that request is adapted, serialized, or forwarded into the router locally or over RPC
- THEN the router-side completion request still represents the same field names and values
- AND no defaulting or serialization behavior in either repo drops those fields silently

### Requirement: OpenAI Codex requests use the Codex Responses transport

The `openai-codex` provider SHALL translate generic completion requests into an OpenAI Codex Responses request sent as `POST https://chatgpt.com/backend-api/codex/responses`, using provider-specific headers instead of `POST /v1/chat/completions`.

#### Scenario: Request body maps prompt, messages, tools, and reasoning

- GIVEN a completion request with a system prompt, conversation history, tool definitions, thinking enabled, and `_session_id`
- WHEN the request is sent to `openai-codex`
- THEN the outbound body is sent to `POST https://chatgpt.com/backend-api/codex/responses`
- AND includes body fields `model`, `store=false`, `stream=true`, `instructions`, `input`, `text={"verbosity":"medium"}` unless the caller explicitly overrides verbosity, `include=["reasoning.encrypted_content"]`, `tool_choice="auto"`, and `parallel_tool_calls=true`
- AND includes body fields `tools` and `reasoning` only when those features are present in the clankers request
- AND includes body field `prompt_cache_key=<_session_id>` when a session key exists

#### Scenario: Request headers carry account and auth metadata

- GIVEN a valid `openai-codex` OAuth access token is loaded
- WHEN the provider sends a request
- THEN it sends header `Authorization: Bearer <access token>`
- AND includes header `chatgpt-account-id` derived from the active token
- AND includes header `OpenAI-Beta: responses=experimental`
- AND includes headers `originator: pi`, `accept: text/event-stream`, and `content-type: application/json`
- AND includes header `session_id: <_session_id>` when `_session_id` is present

#### Scenario: Retry and refresh requests preserve the full normal request contract

- GIVEN an `openai-codex` request is retried after a transient failure or reissued after token refresh
- WHEN the follow-up request is sent
- THEN it still includes headers `Authorization`, `chatgpt-account-id`, `OpenAI-Beta: responses=experimental`, `originator`, `accept`, and `content-type`
- AND it still includes header `session_id` exactly when `_session_id` is present
- AND it preserves the same normal-request body contract, including `store=false`, `stream=true`, `instructions`, `input`, `text`, `include=["reasoning.encrypted_content"]`, and `prompt_cache_key` when `_session_id` is present

### Requirement: Codex entitlement is evaluated through a lightweight probe

The system SHALL evaluate Codex entitlement by sending a lightweight authenticated probe to `POST https://chatgpt.com/backend-api/codex/responses` with headers `Authorization`, `chatgpt-account-id`, `OpenAI-Beta: responses=experimental`, `originator=pi`, and `content-type=application/json`, while explicitly omitting `accept: text/event-stream` and `session_id`. The probe SHALL use body fields `model="gpt-5.1-codex-mini"`, `store=false`, `stream=false`, `instructions="codex entitlement probe"`, `input=[{"role":"user","content":[{"type":"input_text","text":"ping"}]}]`, and `text={"verbosity":"low"}`, omit `tools`, omit `prompt_cache_key`, and omit session-specific state.

#### Scenario: Successful probe marks the account entitled

- GIVEN an `openai-codex` account has valid OAuth credentials and unknown entitlement state
- WHEN the entitlement probe returns a 2xx response from `POST https://chatgpt.com/backend-api/codex/responses`
- THEN clankers marks that account entitled for Codex discovery and request routing

#### Scenario: Unsupported-plan probe marks the account unavailable for Codex

- GIVEN an `openai-codex` account has valid OAuth credentials and unknown entitlement state
- WHEN the entitlement probe returns HTTP 403 or a JSON response body with `error.code = "usage_not_included"`
- THEN clankers treats either signal as authoritative for `not_entitled`
- AND marks that account authenticated but not entitled for Codex use
- AND suppresses `openai-codex` discovery for that account

#### Scenario: Probe transient failures follow normal retry policy

- GIVEN an entitlement probe receives HTTP 401, 429, or retryable 5xx responses
- WHEN clankers handles that probe failure
- THEN HTTP 401 follows the single refresh-and-retry rule
- AND HTTP 429 or retryable 5xx follow the same bounded transient retry policy as normal Codex requests

#### Scenario: Probe cannot establish entitlement after retries

- GIVEN an entitlement probe exhausts its retry budget or returns a non-entitlement provider error
- WHEN clankers still cannot classify the account as entitled or not entitled
- THEN discovery keeps `openai-codex` hidden for that account
- AND provider-aware status shows the account as authenticated with entitlement check failed
- AND explicit or resumed `openai-codex` requests fail closed with a retriable provider error until a later probe succeeds

### Requirement: Prior Codex reasoning signatures are replayed on later turns

The system SHALL preserve the opaque Codex reasoning payload in `Content::Thinking.signature` and replay that payload on later `openai-codex` turns instead of replaying human-readable thinking text.

#### Scenario: Stored reasoning signature is reused on the next turn

- GIVEN an assistant message contains a thinking block with a non-empty `signature`
- WHEN a later `openai-codex` request is built from the conversation history
- THEN the outbound request includes the serialized reasoning payload represented by that signature
- AND does not resend the display-only `thinking` text as prior-turn reasoning input

### Requirement: Codex response streams normalize into generic stream events

The system SHALL map Codex response events into the generic streaming protocol used by clankers so existing turn execution, TUI rendering, and tool handling keep working.

#### Scenario: Text response stream

- GIVEN the Codex backend streams text output
- WHEN the provider parses the stream
- THEN it emits `MessageStart`, `ContentBlockStart(Text)`, `ContentBlockDelta(TextDelta)`, `ContentBlockStop`, `MessageDelta`, and `MessageStop` in order

#### Scenario: Tool call response stream

- GIVEN the Codex backend streams a tool call with incremental arguments
- WHEN the provider parses the stream
- THEN it emits `MessageStart`, `ContentBlockStart(ToolUse)`, `ContentBlockDelta(InputJsonDelta)`, `ContentBlockStop`, `MessageDelta`, and `MessageStop` in order for that logical tool call path

#### Scenario: Reasoning stream with signature retention

- GIVEN the Codex backend streams reasoning or encrypted reasoning signature data
- WHEN the provider parses the stream
- THEN it emits `MessageStart`, `ContentBlockStart(Thinking)`, thinking-related deltas, `ContentBlockStop`, `MessageDelta`, and `MessageStop` in order
- AND does not drop the reasoning signature needed for later turns

#### Scenario: Usage and stop reason are reported

- GIVEN the Codex backend returns token usage and final status
- WHEN the provider finishes parsing the stream
- THEN it records input and output usage in the generic usage structure
- AND maps the final backend status into the generic stop-reason model

#### Scenario: Stream normalization is verified at the real SSE seam

- GIVEN automated coverage validates Codex stream normalization
- WHEN tests exercise the parser path
- THEN at least one test feeds raw SSE bytes through the real Codex parser entrypoint
- AND does not rely only on helper-level event mappers

### Requirement: Retry and auth-failure handling match router expectations

The `openai-codex` provider SHALL retry only HTTP 429, 500, 502, 503, and 504 as transient failures, using at most 3 retries with deterministic backoff delays of 1s, 2s, and 4s and no jitter. HTTP 401 SHALL trigger at most one token refresh followed by one immediate retry. Other non-401 4xx responses SHALL surface directly without retrying.

#### Scenario: Transient failure retries with fixed backoff

- GIVEN the Codex backend returns HTTP 429, 500, 502, 503, or 504
- WHEN the provider handles the response
- THEN it retries the request with delays of 1s, then 2s, then 4s before returning an error
- AND it does not perform more than 3 transient retries

#### Scenario: Unauthorized response refreshes once

- GIVEN the Codex backend returns HTTP 401 and a refresh token is present
- WHEN the provider handles the response
- THEN it performs exactly one refresh
- AND retries the request once with the refreshed token
- AND does not perform more than one refresh cycle for that top-level request

#### Scenario: Non-retryable failure surfaces directly

- GIVEN the Codex backend returns a non-retryable 4xx error other than 401
- WHEN the provider handles the response
- THEN it returns a structured error without retrying
