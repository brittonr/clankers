# Design

## Product-workbench recipe

Create `examples/embedded-product-workbench/` as a standalone Cargo example outside the workspace crate graph. It should depend only on the green embedded SDK crates needed for a local in-process integration: `clanker-message`, `clankers-adapters`, `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, and `serde_json`.

The recipe composes:

- a product-owned in-memory session store with product DTOs and turn receipts,
- a product-owned model adapter implementing `ModelHost`,
- `EmbeddedToolCatalog` + `CatalogToolExecutor` for product-owned tools,
- existing adapter bricks for retry, events, cancellation, and usage observation.

## Scenarios

1. First turn requests a declared product-owned lookup tool, receives correlated tool feedback, finishes, and persists transcript plus receipt.
2. A follow-up turn reloads the product-owned session and proves the provider request includes prior user/tool/assistant context plus the new prompt in deterministic order.
3. Missing session fails closed before model/tool execution and does not create a replacement session.
4. A dangerous tool catalog entry is denied before execution through capability policy.

## Acceptance rail

Wire the example into `scripts/check-embedded-agent-sdk.sh`, update embedded SDK docs, and include the new example directory in the release receipt hash list.
