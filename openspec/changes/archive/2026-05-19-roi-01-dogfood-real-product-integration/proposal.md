# Dogfood real product integration

## Summary

Specify the first real product integration that consumes the green embedded SDK surface without importing Clankers shell/runtime crates.

## Motivation

Clankers already has green embedded SDK crates, adapter bricks, executable examples, capability packs, and release receipts. The next lego-like step needs scoped contracts that make product composition repeatable without collapsing functional-core / imperative-shell boundaries.

## Nickel and BLAKE3 placement

Use Nickel for a product-owned embedding manifest that declares selected crates, capability packs, tool catalog inputs, and shell-exclusion policy. Use BLAKE3 for the checked dogfood receipt: manifest hash, source/example hashes, dependency boundary report hash, and sanitized run transcript hash.

## Non-goals

- Do not move daemon sockets, TUI, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles into generic SDK crates.
- Do not require live credentials, network access, daemon startup, or user-specific local state for acceptance evidence.
- Do not make generic SDK crates evaluate Nickel at runtime; Nickel is an authoring/export/check boundary unless a later change explicitly proves otherwise.
