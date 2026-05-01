## Why

Hermes can run inside ACP-compatible IDEs such as VS Code, Zed, and JetBrains. Clankers has daemon/attach and iroh protocols, but no ACP server/client surface for editor-native chat, tool activity, diffs, and terminal rendering.

## What Changes

- **ACP IDE Integration**: Expose clankers sessions through Agent Client Protocol for ACP-compatible editors.
- **User experience**: Provide a documented CLI/TUI flow and non-interactive mode suitable for daemon and scripted use.
- **Safety and policy**: Respect existing clankers sandboxing, provider credentials, session persistence, and project context boundaries.

## Capabilities

### New Capabilities
- `acp-ide-integration`: Expose clankers sessions through Agent Client Protocol for ACP-compatible editors.

### Modified Capabilities
- `agent-tool-surface`: Agents can use this Hermes-parity feature without bespoke one-off code.
- `session-lifecycle`: Sessions record enough metadata for replay, audit, and troubleshooting.

## Impact

- **Files**: Likely touches `src/tools/`, `src/modes/`, `crates/clankers-agent/`, `crates/clankers-controller/`, `crates/clankers-config/`, docs, and tests.
- **APIs**: Adds or extends user-facing commands/tools/configuration; exact API is finalized during implementation.
- **Dependencies**: May add targeted crates or optional feature-gated integrations.
- **Testing**: Unit tests for parsing/policy, integration tests for session behavior, and docs/examples for the primary path.

## Scope

- **In scope**: A production-ready minimum slice with deterministic tests and documented limitations.
- **Out of scope**: Reimplementing every Hermes provider/backend on the first pass when a local or generic abstraction can land first.
