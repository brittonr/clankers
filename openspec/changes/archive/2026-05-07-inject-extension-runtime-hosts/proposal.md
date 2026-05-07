## Why

The embeddable runtime work separated session, prompt, tool catalog, store, and confirmation seams, but Clankers still has high-value runtime extensions whose ownership is easy to accidentally keep desktop/daemon-local: provider router construction, provider-scoped auth stores, credential pools, and plugin/MCP runtime lifecycle.

Embedding hosts need to choose whether these extension systems are disabled, delegated to Clankers defaults, or supplied by the host. Without an explicit contract, embedders either inherit `~/.clankers`/daemon assumptions or reimplement private provider/plugin setup paths.

## What Changes

- Define a host-owned extension-runtime contract for provider routing, auth/credential stores, and plugin/MCP process runtimes.
- Require default-safe embedding behavior: no router daemon autostart, OAuth flow, credential refresh write, plugin subprocess, MCP server, or gateway startup unless explicitly enabled by the host.
- Require adapter parity fixtures proving normal desktop Clankers defaults still use the same router/auth/plugin behavior through explicit adapters.
- Require safe extension receipts/metadata that identify source/status without leaking credentials, headers, raw plugin payloads, provider request bodies, or environment values.

## Capabilities

### Modified Capabilities
- `embeddable-agent-engine`: Adds host-owned extension service injection for router/auth/plugin lifecycle.
- `tool-host-embedding`: Tightens plugin/MCP publication so extension runtimes are explicit host capabilities, not implicit side effects of catalog construction.

## Impact

- **Files**: Future implementation likely touches `crates/clankers-runtime`, `crates/clankers-provider`, `crates/clanker-router`, plugin runtime modules, `src/modes/common.rs`, and docs.
- **APIs**: Adds host-facing service/adapter traits or structs for extension runtime ownership.
- **Dependencies**: No dependency changes in the spec baseline.
- **Testing**: Strict OpenSpec validation now; later implementation needs parity tests for desktop defaults plus fail-closed embedding tests with extensions disabled.
