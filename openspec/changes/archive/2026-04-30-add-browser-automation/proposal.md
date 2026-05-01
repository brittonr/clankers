## Why

Clankers has a `web` tool for fetching/searching, but not a browser agent that can navigate, click, fill forms, maintain cookies, or extract page state. Hermes supports Browserbase, Browser Use, local Chrome via CDP, and local Chromium.

## What Changes

- **Browser Automation**: Add stateful browser sessions using local Chrome/Chromium CDP first, with room for remote providers later.
- **User experience**: Provide a documented CLI/TUI flow and non-interactive mode suitable for daemon and scripted use.
- **Safety and policy**: Respect existing clankers sandboxing, provider credentials, session persistence, and project context boundaries.

## Capabilities

### New Capabilities
- `browser-automation`: Add stateful browser sessions using local Chrome/Chromium CDP first, with room for remote providers later.

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
