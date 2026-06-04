# Add embedded product-workbench dogfood

## Summary

Add one executable product-style embedded example that composes the green SDK bricks together: a product-owned model adapter, product-owned tool catalog/executor, and product-owned session store/receipt layer. The example should prove a product can build a useful in-process agent from lego-like Clankers crates without importing daemon/TUI/provider-discovery/session-shell runtime surfaces.

## Motivation

Existing examples prove the provider, tool, and session seams independently. Product teams still need a single checked recipe that demonstrates these seams working together in one app-owned integration path before Clankers promotes more reusable public DTO/trait surface.

## Non-goals

- Do not add a reusable public session-store API yet.
- Do not import `clankers-provider`, daemon sockets, TUI, OAuth stores, Clankers DB/session ownership, Matrix, iroh, plugin supervision, or built-in tool bundles into the generic embedded path.
- Do not require live credentials, network access, or external services.
