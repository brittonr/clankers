# Change: Extract Root Tool and Mode Policy Into SDK Bricks

## Problem

The root `clankers` crate still owns too much reusable behavior under `src/tools/`, `src/modes/`, `src/runtime_services.rs`, and `src/slash_commands/`. Root should wire the desktop product, but today many tool policies, slash decisions, runtime adapters, and mode behaviors are trapped in the binary crate and cannot be composed by SDK users without pulling the whole app.

## Goals

- Inventory root modules that own reusable policy instead of wiring/projection.
- Extract at least one high-value root policy into a workspace brick or neutral adapter crate.
- Keep root code as CLI parsing, service assembly, tool registration, and projection.
- Update lego owner receipts for remaining root dependencies.

## Non-goals

- Do not split every root module in one pass.
- Do not remove the root binary or desktop mode behavior.
- Do not move declarative CLI shape solely to reduce file size.

## Proposed scope

Create a root-policy extraction matrix, select one candidate from built-in tools, slash command effects, runtime services, or daemon mode assembly, and move reusable behavior into an owner crate/module with root as adapter. Add rails that distinguish wiring from policy ownership.

## Verification

Validation should include focused behavior tests for the extracted brick, root parity tests, dependency/source rails, and SDK docs/receipt updates when the brick becomes product-facing.
