# Design: Keep Provider and Router Policy at Product Edges

## Summary

Provider/router/auth policy is a desktop/product edge. The reusable SDK should only require a host-owned `ModelHost` or runtime provider service that uses neutral DTOs. This change narrows provider compatibility layers and removes display/protocol concepts from provider-facing contracts.

## Current coupling points

- `crates/clankers-provider/src/lib.rs::CompletionRequest` is provider-native and used by agent adapters.
- `clankers-provider` depends on `clanker-router` and re-exports router/model and retry surfaces.
- `ThinkingLevel` is re-exported from `clanker-tui-types`, leaking display configuration into provider contracts.
- Auth/discovery/credential refresh and request shaping live near compatibility adapters.

## Decisions

### 1. SDK model execution is neutral

Embedders implement `ModelHost` or runtime provider services around their own provider/router. They should not need Clankers OAuth, discovery, cooldown, or provider body builders.

### 2. One owner per concern

Request shaping, auth refresh, discovery, routing/fallback/cooldown, retry, and stream normalization each need one owner. Compatibility adapters translate DTOs and errors only.

### 3. Display DTOs do not cross provider boundaries

Thinking/reasoning settings used by provider adapters should be neutral message/core DTOs; TUI-specific levels stay at display or app-edge projection.

## Validation plan

- Source rails for forbidden TUI/protocol imports and duplicate request-shaping logic.
- Literal provider request fixtures for adapter conversions.
- Embedded provider-adapter example checks that forbid `clankers-provider`, router daemon RPC, OAuth stores, and live network credentials.
