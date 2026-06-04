## Why

Hermes users can type @ references to inject files, folders, git diffs, and URLs directly into a prompt. Clankers has context files and tools, but no inline reference expansion UX or deterministic preflight budget checks.

## What Changes

- **Context References**: Expand @-references in prompts into files, directories, git diffs, URLs, and session artifacts.
- **User experience**: Provide a documented CLI/TUI flow and non-interactive mode suitable for daemon and scripted use.
- **Safety and policy**: Respect existing clankers sandboxing, provider credentials, session persistence, and project context boundaries.

## Capabilities

### New Capabilities
- `context-references`: Expand @-references in prompts into files, directories, git diffs, URLs, and session artifacts.

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
