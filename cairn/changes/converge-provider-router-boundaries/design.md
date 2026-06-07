# Design: Converge Provider Router Boundaries

## Context

The provider boundary already has a bridge module and responsibility inventory. Current duplicate-message-projection counts are zero, but two request/event surfaces remain: `clankers-provider::CompletionRequest` and `clanker-router::CompletionRequest` plus their stream/error compatibility logic. That duplication is acceptable only while adapters stay policy-free and parity rails are strict.

## Decisions

### 1. Name one policy owner per provider concern

**Choice:** Each concern has exactly one owner: body shaping and stream state machines in provider backend modules, routing/fallback/cooldown in router, credential refresh/probe in provider-specific auth owners, cache-key projection in the bridge, and compatibility error mapping in adapters.

**Rationale:** Provider changes become unsafe when the same policy is recreated in local and routed paths. Owner maps turn review memory into a deterministic rail.

### 2. Compatibility adapters translate only

**Choice:** `clankers-provider` adapters may translate messages, content, stream events, metadata, and errors; they must not construct provider-native bodies, run retry/cooldown logic, probe auth, or choose fallback providers.

**Rationale:** The router/backend should own behavior. The compatibility layer exists to preserve the current Clankers API while routing converges.

### 3. Duplicate request DTOs must either collapse or be rail-checked

**Choice:** Prefer collapsing duplicated fields into shared DTOs. If collapse is too disruptive, keep exact constructor-count and shared-field serde projection parity tests.

**Rationale:** The repo has already seen `CompletionRequest` field drift. A rail-backed temporary duplicate is better than a hidden duplicate.

### 4. Fixtures must not call the function under test to build expectations

**Choice:** Wire-contract fixtures pin explicit JSON/request literals for representative history, reasoning, summaries, tool replay, metadata, retry, and refresh behavior.

**Rationale:** Self-derived expected JSON does not catch request-shape drift.

## Risks / Trade-offs

- Full DTO collapse can ripple through tests and root adapters; staged parity rails may be safer.
- Routed backends may need fake provider implementations to avoid live auth/network state.
- Cache-key and request projection are intentionally bridge concerns; keep their scope narrow and fixture-backed.
