# Separate plugin tool runtimes

## Summary

Specify swappable executor/runtime boundaries so Extism, stdio, built-ins, and product catalog tools remain lego-compatible without runtime cross-loading.

## Motivation

Clankers already has green embedded SDK crates, adapter bricks, executable examples, capability packs, and release receipts. The next lego-like step needs scoped contracts that make product composition repeatable without collapsing functional-core / imperative-shell boundaries.

## Nickel and BLAKE3 placement

Use Nickel for runtime-kind allowlists and launch-policy contracts in manifests. Use BLAKE3 for normalized plugin/tool manifests, allowlist exports, and runtime dispatch matrix evidence.

## Non-goals

- Do not move daemon sockets, TUI, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles into generic SDK crates.
- Do not require live credentials, network access, daemon startup, or user-specific local state for acceptance evidence.
- Do not make generic SDK crates evaluate Nickel at runtime; Nickel is an authoring/export/check boundary unless a later change explicitly proves otherwise.
