# Change: Converge Provider Router Boundaries

## Why

`clankers-provider` and workspace-local `clanker-router` still both expose provider-facing request/event abstractions. Recent rails keep projections in sync, but request shaping, auth probing, discovery, retry, cooldown, cache-key, and stream normalization can still drift if compatibility adapters grow policy again. This is too coupled because each provider change must be audited across two abstractions.

## What Changes

- Produce a concern owner map for provider-native body shaping, auth refresh/probe, discovery, routing/fallback/cooldown, retry, cache keys, and stream normalization.
- Keep `clankers-provider` compatibility adapters as DTO/error/stream projection only, delegating policy to the declared owner.
- Collapse or narrow duplicate `CompletionRequest`/stream abstractions where practical, or enforce constructor-count and shared-field parity rails for the remaining compatibility layer.
- Add fixture-backed tests that prove router-backed and RPC-backed adapters preserve branch/compaction summaries and request metadata without duplicating backend policy.

## Impact

- **Files**: `crates/clankers-provider/src/{router.rs,rpc_provider.rs,router_request_bridge.rs,provider_router_responsibility.rs,lib.rs}`, `crates/clanker-router/**`, provider request-shape tests, and ownership rails.
- **Testing**: provider/router responsibility rail, request fixture tests, constructor-count parity tests, routed backend smoke with fake providers, `cargo check --tests`, Cairn gates, and diff checks.
