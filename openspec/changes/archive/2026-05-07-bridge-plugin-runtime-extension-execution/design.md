## Context

`ExtensionServices` made provider/router/auth/plugin/MCP/gateway systems explicit and disabled by default for embedded runtime construction. The desktop adapter still used receipt-only stubs for plugin runtime behavior.

## Decision

Add a plugin-manager-backed desktop `ExtensionRuntimeService` constructor while keeping the existing no-plugin `from_paths` behavior as a safe capability-only adapter.

The plugin runtime request carries safe execution routing fields (`extension_name`, `runtime_entrypoint`) plus JSON arguments. The desktop adapter uses those fields to call an injected `PluginManager` for WASM plugins and records only safe receipt metadata such as plugin name, visible tool name, handler name, status, and output byte count.

## Non-Goals

- No provider/router async model execution is added in this slice.
- No stdio plugin process lifecycle is routed through this seam yet.
- No raw plugin input/output is stored in receipts.

## Risks

- The request contract can grow too plugin-specific. Mitigation: keep fields generic as extension/runtime entrypoint names and arguments, and leave provider/router under the separate provider service trait.
- Receipt metadata could leak raw plugin content. Mitigation: record byte counts and identifiers only.
