Artifact-Type: oracle-checkpoint
Evidence-ID: architecture-oracle
Task-ID: H1
Covers: embeddable.agent.engine.reusableembeddableenginecrate.enginecratedefinesahostfirstboundary, embeddable.agent.engine.reusableembeddableenginecrate.enginecrateislayeredaboveclankerscore, embeddable.agent.engine.messageevolutionpolicy.messagepolicyremainsindependentofpromptassembly, embeddable.agent.engine.appspecificconcerns.systempromptassemblystaysappspecific, embeddable.agent.engine.appspecificconcerns.transportanduiconcernsstayappspecific

## Question
Does the target architecture keep prompt assembly, transport, and UI concerns outside the proposed `clankers-engine` boundary while routing reusable host-facing harness semantics through the engine first?

## Architectural Facts Reviewed
- `clankers-core` is already the low-level deterministic FCIS/no-std layer and must stay plain-data and shell-agnostic.
- `clankers-agent` currently owns the strongest reusable turn semantics in `crates/clankers-agent/src/turn/mod.rs`: prompt -> model -> tool -> continuation, retry, cancellation, and stop handling.
- `clankers-controller` currently owns shell translation/boundary enforcement and is the right place to remain authoritative for daemon/session translation, not reusable turn policy.
- Prompt assembly lives outside this reusable turn slice today (`crates/clankers-agent/src/system_prompt.rs`) and should remain outside the future engine boundary.
- TUI, attach, daemon framing, and embedded runtime concerns live in app-shell code under `src/modes/` and related UI crates, and should remain shell-only.

## Oracle Judgment
Yes. The proposed architecture is coherent if and only if `clankers-engine` becomes the first host-facing landing zone for reusable turn semantics that are currently trapped in `clankers-agent::turn`, while `clankers-controller`, `clankers-agent`, and app shells are reduced to adapters around that engine.

## Why This Boundary Is Correct
- Embedders need one reusable harness contract, not a controller-shaped or daemon-shaped API. A new `clankers-engine` layer provides that without polluting `clankers-core` with host/runtime concerns.
- Prompt assembly is product policy, not reusable harness policy. AGENTS.md, SYSTEM.md, APPEND_SYSTEM.md, OpenSpec, skills, and project-context discovery should continue producing already-prepared prompt inputs outside the engine.
- Transport and UI concerns are shell-specific render/protocol choices. `DaemonEvent`, attach flow, TUI widgets, terminal loops, and runtime channels should remain translations from engine-native semantic events rather than engine API types.
- `clankers-agent` should still execute provider/tool I/O, but after the split it must do so because engine effects requested that work, not because the async runtime still owns the continuation state machine.
- `clankers-controller` should still own session persistence and daemon/session translation, but it should stop being the long-term home of reusable prompt/model/tool policy.

## Risks To Watch
- Split-brain migration: controller/agent may temporarily keep shadow copies of continuation logic. The parity rails are mandatory to detect this.
- Over-expanding engine scope: if prompt discovery, hooks policy, daemon framing, or TUI state enter engine contracts, the boundary will become app-specific and lose embeddability.
- Under-specifying correlation: engine-owned request/call IDs must stay explicit and host-echoed, or adapters will reintroduce ambient runtime coupling.

## Decision
Proceed with the `clankers-engine` architecture exactly as a host-facing reusable harness layer above `clankers-core` and below controller/agent/app shells.

## Required Follow-Through
- Add the `clankers-engine` crate to the workspace.
- Add a public-surface rail specific to that crate.
- Migrate the reusable turn state machine from `clankers-agent::turn` into engine-owned contracts.
- Keep prompt assembly and all transport/UI concerns outside the engine boundary.
