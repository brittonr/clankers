## Why

Clankers has a verified embedded SDK boundary, but product embedders still have to assemble low-level engine, host, tool, event, usage, cancellation, and catalog pieces by hand. That makes the architecture technically embeddable but not yet "lego-like": small safe pieces are present, while reusable product kits, adapter bricks, declarative tool composition, and executable recipes are incomplete.

## What Changes

- **Reusable adapter bricks**: Add a shell-free adapter crate that provides boring host-owned implementations for common embedding needs such as memory event sinks, cancellation, retry sleeping, usage collection, and fake/scripted model hosts.
- **Composition kits**: Define product-facing bundle patterns that assemble SDK crates and adapter bricks without importing daemon/TUI/provider-discovery/database/runtime shells.
- **Declarative tool catalogs**: Specify a small, validated catalog format that maps declarative tool descriptors into `ToolCatalog`/capability data while failing closed on dangerous defaults.
- **Capability-pack presets**: Specify safe, named capability packs for product embeddings.
- **Executable recipes and docs**: Add checked examples and crate guidance that distinguish green/yellow/red product surfaces.

## Capabilities

### New Capabilities

- `embedded-composition-kits`: Product embedders can start from verified kits and adapter bricks instead of wiring every trait manually.
- `embedded-tool-catalogs`: Product embedders can declare tool availability and capability policy in data and receive deterministic validation.
- `embedded-capability-packs`: Product embedders can select named safe capability packs with regression coverage.

### Modified Capabilities

- `embeddable-agent-engine`: Extends the existing embeddable engine path with reusable composition layers while preserving the pure engine/host/tool/message boundary.

## Impact

- **Files**: Expected changes under `crates/clankers-adapters/`, optional kit modules or crate, `crates/clankers-runtime`/tool-catalog seams, `examples/embedded-*`, `docs/src/tutorials/embedded-agent-sdk.md`, generated docs/checkers, and `scripts/check-embedded-agent-sdk.sh`.
- **APIs**: New public adapter/kit APIs must be documented in the embedded SDK inventory or explicitly marked internal.
- **Dependencies**: Adapter/kit crates must not pull daemon, TUI, provider discovery, router daemon RPC, database, Matrix, iroh/P2P, plugin supervision, or concrete Clankers runtime shells into the generic embedded path.
- **Testing**: Extend embedded SDK acceptance to compile/run recipes, deny forbidden dependency/source leakage, validate catalog negative cases, and prove capability packs fail closed.

## Out of Scope

- Stabilizing a public third-party semver promise beyond the documented supported entrypoints.
- Moving provider discovery, OAuth stores, daemon sockets, TUI, Matrix, iroh, plugin supervision, or session DB ownership into the generic SDK path.
- Shipping product-specific UI, web server, or cloud deployment integrations.
- Replacing existing daemon/MCP/ACP integration surfaces.
