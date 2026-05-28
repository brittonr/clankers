# Design: Thin Root and Controller Runtime Adapters

## Summary

Root and controller can remain product-shell crates, but they should not be where reusable policy lives. They should wire service bundles, translate input/output, and call runtime/controller bricks with clear ownership receipts for any concrete dependency.

## Decisions

### Decision: root is composition only

Root modules may parse CLI, initialize desktop services, choose modes, and wire adapters. Reusable request shaping, session persistence policy, rendering semantics, provider routing, and tool execution policy should live in workspace crates or named modules with tests.

### Decision: controller delegates execution to runtime/session services

Controller should own command lifecycle and transport-agnostic orchestration, but not concrete provider/db/config/session/protocol/TUI policy inline. Runtime execution should flow through services and semantic event projection.

### Decision: dependency budgets are enforced with receipts

The architecture rail should track root/controller internal dependency counts, owner reasons, and adapter names. New dependencies require explicit owner receipts or fail validation.

## Verification Plan

- Extend dependency ownership inventory with root/controller dependency budgets and adapter ownership.
- Add controller fixtures that operate with fake runtime/session services and no provider/db/TUI/protocol construction.
- Keep daemon/local/remote attach parity tests green.
