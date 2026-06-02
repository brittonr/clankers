# Change: Provider Router Abstraction Collapse

## Problem

`clankers-provider` still defines provider-facing `Provider`, `CompletionRequest`, credential helpers, and stream/error adapters while `clanker-router` owns similar request, auth, routing, fallback, cooldown, and backend policy. The compatibility layer is useful, but duplicate abstractions still create request-shape drift risk.

## Goals

- Inventory duplicate provider/router abstractions by concern.
- Collapse one duplicate request, stream, auth, or discovery concern to a single owner.
- Keep adapters thin and covered by literal request/stream fixtures.

## Non-goals

- Do not break public provider APIs without a compatibility path.
- Do not build expected fixtures by calling the same body builder under test.
- Do not move provider-native backend policy out of router/backend owners.

## Proposed scope

Start with one duplicated concern, such as `CompletionRequest` shape, stream event normalization, auth credential manager wrappers, or discovery probing, and delegate policy to the single owner with adapter-only conversion tests.

## Verification

Focused validation should include provider/router adapter fixtures, constructor-count/parity rails, cargo check for provider/router users, Cairn gates, and `git diff --check`.
