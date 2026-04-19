## Why

clankers can use Claude subscription accounts through Anthropic OAuth compatibility, but OpenAI support is still API-key-only and goes through the Chat Completions path. Users with ChatGPT Plus or Pro subscriptions cannot authenticate those plans, cannot select Codex models, and cannot benefit from the subscription-backed workflow that OpenAI exposes through its Codex Responses endpoint.

The current seams are already visible in the codebase:
- `src/commands/auth.rs` and `src/slash_commands/handlers/auth.rs` accept provider-shaped inputs but hardcode Anthropic login behavior.
- `crates/clankers-provider/src/discovery.rs` only builds OpenAI from API keys and the generic `OpenAICompatProvider`.
- `crates/clankers-provider::CompletionRequest` drops provider-specific metadata such as stable session identifiers before the router layer sees the request.

Supporting OpenAI subscription plans needs a dedicated provider path, not more special cases in the existing API-key `openai` backend.

## What Changes

- Add provider-aware OAuth plumbing so `openai-codex` can be authenticated, refreshed, listed, switched, and logged out alongside Anthropic.
- Add a dedicated OpenAI Codex subscription backend that talks to the ChatGPT Codex Responses endpoint instead of the existing Chat Completions API-key path.
- Extend request plumbing to carry provider-specific extra parameters and a stable session key into router backends.
- Add discovery and model catalog entries for `openai-codex`, while keeping API-key `openai` behavior unchanged.
- Evaluate Codex entitlement separately from raw OAuth success so unsupported accounts stay authenticated but cannot route `openai-codex` turns.

## Capabilities

### New Capabilities

- `openai-codex-auth`: Authenticate ChatGPT Plus/Pro subscriptions as provider `openai-codex`, store them in the shared auth store, and auto-refresh them.
- `openai-codex-responses`: Send clankers turns through the ChatGPT Codex Responses transport with tool use, thinking, retry, and usage normalization.
- `openai-codex-model-selection`: Discover Codex subscription models as a separate provider family, suppress unsupported accounts from that catalog, and route requests without mutating API-key `openai` semantics.

### Modified Capabilities

- `provider-auth`: Auth commands and slash commands become provider-aware instead of Anthropic-only.
- `provider-request-plumbing`: Provider-specific metadata can survive the clankers -> router boundary.

## Impact

- `src/commands/auth.rs`
- `src/slash_commands/handlers/auth.rs`
- `src/modes/interactive.rs`
- `src/modes/agent_task.rs`
- `src/worktree/llm_resolver.rs`
- `crates/clankers-agent/src/turn/execution.rs`
- `crates/clankers-provider/src/{auth.rs,discovery.rs,lib.rs,router.rs,rpc_provider.rs}`
- `../clanker-router/src/{auth.rs,oauth.rs,provider.rs}` plus a new OpenAI Codex backend module and related tests
- Model catalog / registry entries for `openai-codex`
- User-facing auth and provider documentation

## Non-Goals

- Changing existing Anthropic subscription behavior.
- Replacing API-key `openai` with the Responses API.
- Adding WebSocket transport in the first change.
- Supporting non-Codex consumer ChatGPT endpoints.

## Verification

- Unit tests for provider-aware auth, account-ID derivation, refresh, switch, logout, and omitted-provider Anthropic defaults.
- A deterministic constructor-inventory check over every router-bound `CompletionRequest` creation site touched by this change, including `crates/clankers-agent/src/turn/execution.rs`, `src/modes/agent_task.rs`, `src/worktree/llm_resolver.rs`, `crates/clankers-provider/src/router.rs`, and `crates/clankers-provider/src/rpc_provider.rs`, so `extra_params`, including the `_session_id` key inside that map, cannot be omitted silently.
- A deterministic cross-repo parity check proving `crates/clankers-provider::CompletionRequest` and `clanker-router::CompletionRequest` both preserve and serialize `extra_params`, including the `_session_id` key, across local and RPC execution paths.
- Discovery and model-resolution tests with entitled, missing, and unsupported `openai-codex` accounts, including user-visible provider-qualified catalog entries, request-time blocking for explicit or resumed `openai-codex` use by unsupported accounts, and deterministic regression coverage proving API-key `openai` behavior stays unchanged.
- A deterministic request-fixture check that every Codex request path, including entitlement probes, initial requests, transient retries, and 401 refresh retries, sends the required Codex headers (`Authorization`, `chatgpt-account-id`, `OpenAI-Beta: responses=experimental`, and the required request-specific transport headers, including probe `accept: text/event-stream` while still omitting `session_id`) with the correct auth/account context and expected request body flags for normal requests versus entitlement probes. These checks use pinned literal JSON/header fixtures, not expectations derived by calling the same request builders under test.
- At least one real SSE seam test for Codex stream normalization, not just helper-only parser tests.
- One manual live-credential smoke path against a real ChatGPT Plus/Pro subscription after fixture coverage passes, using an already-authenticated account to verify live status/account switching, model resolution, and one Codex turn. Fresh login and interactive reload remain covered by recorded-fixture and automated auth tests.
- A docs acceptance check that CLI help, slash help, and provider docs describe `openai-codex` login, model selection, personal-use/plan limitations, and any unsupported-plan behavior.

## Coordination / Ownership

This change explicitly includes coordinated work in the extracted sibling repo `../clanker-router`. The clankers OpenSpec owns the end-to-end behavior, but implementation is expected to land as:
- router-side auth/backend changes in `../clanker-router`
- a workspace pin update back in this repo
- final validation from this repo via `cargo nextest run`, `cargo clippy -- -D warnings`, and `nix build .#clankers`
- the implementer of the coordinated change runs the manual live-credential smoke path against an already-authenticated real subscription account and records pass/fail evidence in the implementation PR or change notes before the change is considered ready
