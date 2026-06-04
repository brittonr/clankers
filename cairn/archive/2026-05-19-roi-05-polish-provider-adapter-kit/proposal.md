# Polish provider adapter kit

## Summary

Improve product-owned provider adaptation examples without pulling clankers-provider, OAuth, or discovery into generic SDK crates.

## Motivation

Clankers already has green embedded SDK crates, adapter bricks, executable examples, capability packs, and release receipts. The next lego-like step needs scoped contracts that make product composition repeatable without collapsing functional-core / imperative-shell boundaries.

## Nickel and BLAKE3 placement

Use Nickel only for optional example request-profile fixtures and model capability declarations. Use BLAKE3 for request/response fixture hashes and adapter receipt envelopes proving retry/terminal-failure semantics.

## Non-goals

- Do not move daemon sockets, TUI, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles into generic SDK crates.
- Do not require live credentials, network access, daemon startup, or user-specific local state for acceptance evidence.
- Do not make generic SDK crates evaluate Nickel at runtime; Nickel is an authoring/export/check boundary unless a later change explicitly proves otherwise.
