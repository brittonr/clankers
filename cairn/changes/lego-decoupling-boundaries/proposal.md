# Proposal: Lego Decoupling Boundaries

## Problem

Clankers has several coupling hotspots that make features harder to compose as reusable bricks: the root crate acts as a god-shell, `clankers-agent` knows concrete runtime/provider/storage systems, controller seams combine translation and policy, process jobs mix tool JSON/runtime/storage concerns, provider routing has overlapping layers, TUI/protocol event types leak across domains, attach parity is duplicated across transports, and architecture contracts are enforced mostly by brittle string rails.

## Proposed Change

Add a native Cairn architecture package that defines the target lego boundaries and the verification rails needed to keep them from regressing. This package is planning/specification only: it does not change runtime behavior directly, but it creates accepted requirements for future extraction work.

## Impact

- Future decoupling work can land in narrow slices with stable requirement IDs.
- New features must preserve root-shell thinness, agent port boundaries, controller translation/effect separation, process-job adapter thinness, provider/router ownership, event DTO neutrality, shared session-command parity, and typed architecture rails.
- Existing behavior remains compatible while implementations migrate behind explicit adapters and fixtures.
