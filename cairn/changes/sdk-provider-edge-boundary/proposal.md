# Change: Keep Provider and Router Policy at Product Edges

## Problem

`clankers-provider` is not SDK-clean: it wraps provider traits, concrete request DTOs, router compatibility, OAuth stores, discovery, credential refresh, streaming normalization, and a TUI-owned `ThinkingLevel` re-export. SDK users should provide model adapters over `ModelHost`, not import Clankers desktop provider/router/auth policy.

## Goals

- Keep provider/router/auth as red app-edge dependencies for generic SDK paths.
- Move provider-native request shaping and router bridging to one owner per concern.
- Replace display DTO leakage in provider-facing APIs with neutral message/core DTOs.
- Provide product-owned provider adapter fixtures that do not derive expected bodies from the implementation under test.

## Non-goals

- Do not rewrite all live provider backends.
- Do not remove desktop OAuth login/discovery behavior.
- Do not require embedders to use Clankers provider/router crates.

## Proposed scope

Define a stricter provider-edge boundary: generic SDK crates expose neutral model-host requests/events, while desktop adapters bridge to `clankers-provider`/`clanker-router`. Inventory duplicate request/event abstractions and collapse or rail them with parity fixtures.

## Verification

Validation should include provider-adapter fixtures, request-shape parity rails, no-TUI DTO checks in provider APIs, and embedded SDK dependency denylist checks.
