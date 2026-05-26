# Change: Daemon attach reconnect dogfood

## Why

Operator-visible daemon attach/reconnect parity is a valuable next dogfood seam after BG-process TUI coverage, but it is broader and should be planned before implementation. The risk is stale suppression state, lost replay, or confusing attach UI after reconnect.

## What Changes

- Define a deterministic dogfood or harness rail for local daemon attach/reconnect behavior.
- Cover session creation, attach, slash/action parity state, reconnect reset, and history replay visibility.
- Keep remote QUIC attach as an optional later extension unless the local seam is stable.

## Non-Goals

- No remote iroh/QUIC coverage in the first slice unless explicitly enabled.
- No broad daemon architecture rewrite.
- No reliance on real model credentials for reconnect proof.
