## Why

Clankers now has a real `no_std` functional core, but the extracted surface is still too narrow and controller-shaped to serve as a reusable embedded agent harness. Projects that want to embed Clankers-style agent execution still have to adopt large parts of the current controller, agent, provider, and app shell stack instead of depending on one small host-facing engine crate.

This change defines the next extraction milestone: a stable embeddable engine API layered above `clankers-core`. The goal is to let other projects drive prompt, model, tool, and continuation flows through a compact host-facing contract while Clankers itself reuses the same engine internally.

## What Changes

- Introduce a new embeddable engine capability that defines a host-first `clankers-engine` crate above `clankers-core`.
- Define the canonical engine boundary in terms of plain engine state, engine inputs, engine effects, engine outcomes, and host feedback re-entry rather than daemon, TUI, or controller protocol types.
- Extract the highest-value reusable orchestration slice next: end-to-end turn execution policy for prompt submission, model completion handling, tool-call planning, tool-result ingestion, continuation, retry, and stop decisions.
- Split current controller and agent responsibilities so Clankers app shells become adapters over the engine instead of the place where harness semantics live.
- Keep system-prompt assembly, daemon protocol framing, TUI behavior, and other app/runtime concerns outside the engine boundary.

## Capabilities

### New Capabilities
- `embeddable-agent-engine`: Define the reusable host-facing engine crate, API contracts, and migration path for turning Clankers into an embeddable agent harness.

### Modified Capabilities
- `no-std-functional-core`: Extend the functional-core roadmap so future FCIS extraction work targets the `clankers-engine` host boundary and the turn-orchestration slice needed for embedding.

## Impact

- Affected code: `crates/clankers-core`, `crates/clankers-controller`, `crates/clankers-agent`, new `crates/clankers-engine`, and the app shells in `src/modes/`.
- APIs: adds a new public embedding surface and constrains future controller/agent extraction work to flow through it.
- Architecture: splits reusable engine semantics from Clankers-specific daemon, TUI, prompt-assembly, hook, and protocol shells.
- Testing: requires new engine-focused deterministic, host-adapter, and parity rails in addition to the existing `no-std-functional-core` validation bundle.
