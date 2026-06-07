# Change: Converge Provider Router Owners

## Why

`clankers-provider` and `clanker-router` still share provider concerns through compatibility adapters and duplicated request/event abstractions. That split is a drift risk: adding a request field, retry rule, auth probe, cache key, or stream event can silently update one side while the other side keeps stale behavior.

## What Changes

- Inventory provider/router concerns and declare one owner for request shaping, routing/fallback/cooldown, auth refresh/probing, model/account discovery, cache-key projection, retry policy, and stream normalization.
- Keep `clankers-provider` compatibility adapters thin: DTO conversion, stream/error translation, and explicit compatibility fixtures only.
- Collapse or delegate at least one duplicated concern to the declared owner and remove duplicate policy from the compatibility layer.
- Extend parity rails so constructor counts, request projections, stream normalization, and cache-key projections fail when the duplicate abstractions drift.

## Impact

- **Files**: `crates/clankers-provider/src/{router.rs,rpc_provider.rs,router_request_bridge.rs,provider_router_responsibility.rs}`, workspace-local `crates/clanker-router`, provider contract tests, and embedded SDK compatibility docs.
- **Testing**: provider-router boundary rail, request-shape literal fixtures, stream parser runtime seam tests where touched, constructor-count parity tests, and Cairn validation/gates.
