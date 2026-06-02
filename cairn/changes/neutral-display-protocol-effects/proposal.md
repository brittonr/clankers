# Change: Neutral Display and Protocol Effects

## Problem

Slash, attach, and mode policy still mix reusable decisions with `clanker-tui-types` display DTOs and `clankers_protocol::SessionCommand` constructors. Shared slash/effect policy exists, but protocol and display shapes still leak into policy modules that should be neutral.

## Goals

- Introduce or extend neutral effect DTOs for reusable slash/mode policy.
- Move TUI and protocol construction to projection adapters.
- Keep standalone, local attach, and remote attach parity covered by deterministic tests.

## Non-goals

- Do not remove TUI or protocol DTOs from projection edges.
- Do not change slash command names or daemon protocol semantics.
- Do not rewrite every slash handler in one pass.

## Proposed scope

Drain one policy family, such as thinking/disabled-tools/model/plugin slash effects or loop display state, so reusable policy returns neutral effects and edge adapters project to TUI/protocol DTOs.

## Verification

Focused validation should include slash parity tests, attach/local/remote command tests, source-boundary rails, Cairn gates, and `git diff --check`.
