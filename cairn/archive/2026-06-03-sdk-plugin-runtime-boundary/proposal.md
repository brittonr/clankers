# Change: Separate Plugin Runtime, Manifest, Tool, and UI Boundaries

## Problem

`clankers-plugin` mixes Extism loading, stdio supervision, sandbox policy, manifest discovery, host event queues, tool registration, hooks, and UI types. It also depends on `clanker-tui-types`. SDK users need plugin manifest/tool-runtime concepts without inheriting desktop TUI/plugin-supervisor coupling.

## Goals

- Split manifest/catalog validation from runtime supervision and UI projection.
- Keep Extism, stdio, built-in, and product-owned runtime dispatch owners separate.
- Replace plugin UI dependencies with neutral events at the plugin-core boundary.
- Keep desktop plugin manager as an app-edge composition.

## Non-goals

- Do not remove Extism or stdio plugin support.
- Do not change plugin manifest format unless migration notes and fixtures are updated.
- Do not promote desktop plugin supervision into green SDK crates.

## Proposed scope

Inventory `clankers-plugin` responsibilities, extract or isolate manifest/tool runtime dispatch contracts from TUI projection and supervisor state, and update plugin-runtime-dispatch rails so non-Extism runtimes never flow through eager WASM loading or UI-only types.

## Verification

Validation should include manifest fixtures, runtime dispatch matrix checks, source rails for TUI/protocol leakage, stdio/Extism parity tests, and SDK dependency denylist checks.
