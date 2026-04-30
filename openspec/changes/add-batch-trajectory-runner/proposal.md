## Why

Hermes supports batch processing over many prompts and ShareGPT-format trajectory generation for eval/RL data. Clankers has subagents and daemon sessions, but no first-class batch runner with bounded concurrency, resumability, and trajectory export.

## What Changes

- **Batch Processing and Trajectory Export**: Run many prompts concurrently and export structured trajectories for evaluation and training.
- **User experience**: Provide a documented CLI/TUI flow and non-interactive mode suitable for daemon and scripted use.
- **Safety and policy**: Respect existing clankers sandboxing, provider credentials, session persistence, and project context boundaries.

## Capabilities

### New Capabilities
- `batch-processing-and-trajectory-export`: Run many prompts concurrently and export structured trajectories for evaluation and training.

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
