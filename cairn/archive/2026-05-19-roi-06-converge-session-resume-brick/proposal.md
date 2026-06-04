# Converge session resume brick

## Summary

Collect enough dogfood evidence to decide whether host-owned session/resume DTOs should become a reusable brick.

## Motivation

Clankers already has green embedded SDK crates, adapter bricks, executable examples, capability packs, and release receipts. The next lego-like step needs scoped contracts that make product composition repeatable without collapsing functional-core / imperative-shell boundaries.

## Nickel and BLAKE3 placement

Use Nickel for optional product session-schema contract examples and migration policy. Use BLAKE3 for transcript/receipt hashes, restored-context fixtures, and deterministic session export/import evidence.

## Non-goals

- Do not move daemon sockets, TUI, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles into generic SDK crates.
- Do not require live credentials, network access, daemon startup, or user-specific local state for acceptance evidence.
- Do not make generic SDK crates evaluate Nickel at runtime; Nickel is an authoring/export/check boundary unless a later change explicitly proves otherwise.
