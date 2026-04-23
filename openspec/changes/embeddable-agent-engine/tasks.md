## 1. Engine contract and architecture split

- [ ] I1 Define the new `embeddable-agent-engine` capability around a workspace crate named `clankers-engine`, and spell out the host-facing `EngineState`, `EngineInput`, `EngineEffect`, `EngineOutcome`, and `EngineEvent` contract in terms of plain engine-native data rather than daemon, TUI, or interactive-mode protocol types.
- [ ] I2 Define the explicit host-driven execution contract for user-prompt submission, model-request effects, model-result feedback, tool-execution effects, tool-result feedback, and semantic engine events so hosts drive execution by executing correlated engine effects and feeding correlated engine inputs back.
- [ ] I3 Update the `no-std-functional-core` capability so future reusable orchestration extractions are staged through `clankers-engine`, while pure deterministic logic remains eligible for later downward migration into `clankers-core`.
- [ ] I4 Document the crate map and ownership split across `clankers-core`, `clankers-engine`, `clankers-controller`, `clankers-agent`, `clankers-provider`, `clankers-message`, and app shells so future work has one canonical target architecture for embedding.

## 2. Engine-owned reusable turn and message slice

- [ ] I5 Define the end-to-end turn-orchestration slice that must move next from `clankers-agent`, explicitly covering prompt submission, model completion handling, tool-call planning, tool-result ingestion, retry decisions, cancellation, token-limit handling, and terminal stop behavior as engine-owned reusable policy.
- [ ] I6 Define the reusable conversation/message evolution rules that the engine owns for user input, assistant/model output, tool results, and continuation ordering, while keeping AGENTS.md / SYSTEM.md / APPEND_SYSTEM.md / OpenSpec / skills prompt assembly outside the engine boundary.
- [ ] I7 Define the post-split responsibilities of `clankers-controller` and `clankers-agent` as adapters over `clankers-engine`, including which prompt, model, tool, retry, and continuation behaviors must stop being authoritative outside the engine and which transport, UI, hook, provider, and prompt-assembly concerns remain shell-only.

## 3. Verification and oracle rails

- [ ] V1 Define verification rails that reject leakage of `DaemonEvent`, `SessionCommand`, TUI runtime/widget types, and other app-protocol or UI concerns from the public `clankers-engine` surface. [evidence=openspec/changes/embeddable-agent-engine/evidence/engine-surface-rail.md]
- [ ] V2 Define parity rails proving `clankers-controller` and `clankers-agent` execute engine-requested model/tool work, translate semantic engine events, and feed correlated host results back without re-deriving reusable turn policy locally. [evidence=openspec/changes/embeddable-agent-engine/evidence/adapter-parity-rail.md]
- [ ] V3 Define positive and negative engine turn-state-machine rails covering prompt submission, model completion, tool-request planning, tool-result ingestion, retry, cancellation, token-limit handling, and terminal stop behavior for the migrated reusable slice. [evidence=openspec/changes/embeddable-agent-engine/evidence/turn-state-machine-rail.md]
- [ ] H1 Perform an oracle architecture review over `clankers-core`, the proposed `clankers-engine`, `clankers-controller`, `clankers-agent`, and app-shell boundaries to confirm the target architecture keeps prompt assembly, transport, and UI concerns outside the engine while routing reusable harness semantics through the engine boundary. [evidence=openspec/changes/embeddable-agent-engine/evidence/architecture-oracle.md]

## 4. Traceability matrix

- `embeddable-agent-engine` crate boundary and layering requirement → `I1`, `I4`, `H1`
- explicit host-driven execution contracts → `I2`, `V2`, `V3`
- engine-owned turn orchestration → `I5`, `V2`, `V3`
- engine-owned message evolution with prompt-assembly kept outside engine → `I6`, `H1`
- controller/agent adapters over engine → `I7`, `V2`, `H1`
- app-specific concerns stay outside engine → `I6`, `I7`, `V1`, `H1`
- embedding-focused migration rails → `V1`, `V2`, `V3`, `H1`
- `no-std-functional-core` future extraction staging through engine → `I3`, `H1`
