## Why

The host-owned extension runtime seam currently proves disabled defaults and desktop capability receipts, but no real plugin execution path is exercised through that seam. Embedders need one concrete adapter path that demonstrates plugin publication and invocation can be hosted explicitly without falling back to hidden ambient behavior.

## What Changes

- Add a focused desktop plugin runtime adapter behind `ExtensionRuntimeService`.
- Publish plugin tool descriptors from an injected `PluginManager` through the runtime seam.
- Execute a real plugin tool through the runtime seam and return safe receipt metadata.

## Scope

In scope: WASM plugin publication/execution through the runtime extension service, safe receipt metadata, and regression tests using the existing test plugin.

Out of scope: provider/router model completion, stdio plugin lifecycle routing, MCP/gateway execution routing, and changing normal CLI/TUI plugin behavior.

## Verification

Run focused runtime-service/plugin tests, compile checks for `clankers-runtime` and `clankers`, strict OpenSpec validation, and whitespace checks.
