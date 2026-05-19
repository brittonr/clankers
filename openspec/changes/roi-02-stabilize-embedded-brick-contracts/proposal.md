# Stabilize embedded brick contracts

## Summary

Define the semver, dependency-boundary, and generated inventory rules that make green SDK crates reliable lego bricks.

## Motivation

Clankers already has green embedded SDK crates, adapter bricks, executable examples, capability packs, and release receipts. The next lego-like step needs scoped contracts that make product composition repeatable without collapsing functional-core / imperative-shell boundaries.

## Nickel and BLAKE3 placement

Use Nickel as the source of truth for the green/yellow/red crate contract and denylist vocabulary. Use BLAKE3 in release receipts to pin generated API inventories, boundary reports, and crate contract exports.

## Non-goals

- Do not move daemon sockets, TUI, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles into generic SDK crates.
- Do not require live credentials, network access, daemon startup, or user-specific local state for acceptance evidence.
- Do not make generic SDK crates evaluate Nickel at runtime; Nickel is an authoring/export/check boundary unless a later change explicitly proves otherwise.
