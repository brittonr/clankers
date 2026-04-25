## ADDED Requirements

### Requirement: Core and engine reducers MUST have explicit ownership boundaries

The system MUST keep `clankers-core` and `clankers-engine` as independently understandable reducers with explicit adapter composition rather than implicit state pass-through.
r[embeddable-agent-engine.core-engine-ownership]

#### Scenario: core-owned lifecycle policy stays in clankers-core
r[embeddable-agent-engine.core-owned-policy]
- **WHEN** behavior concerns prompt lifecycle, queued prompt replay, loop follow-up dispatch/completion, auto-test follow-up dispatch/completion, thinking-level changes, or disabled-tool filter state
- **THEN** the authoritative deterministic policy lives in `clankers-core`
- **THEN** controller and agent shells execute core effects rather than duplicating that policy locally or moving it into the engine turn reducer

#### Scenario: engine-owned turn policy stays in clankers-engine
r[embeddable-agent-engine.engine-owned-policy]
- **WHEN** behavior concerns model request correlation, model completion, tool-call planning, tool feedback ingestion, retry scheduling, continuation budget, cancellation during model/tool/retry phases, or terminal turn outcomes
- **THEN** the authoritative deterministic policy lives in `clankers-engine`
- **THEN** controller and agent shells execute engine effects rather than duplicating that policy locally or moving it into `clankers-core`

### Requirement: Engine state MUST NOT carry dormant core state

The engine state MUST NOT include `CoreState` or other core reducer state as an unused pass-through field.
r[embeddable-agent-engine.no-dormant-core-state]

#### Scenario: engine state contains only active turn data
r[embeddable-agent-engine.engine-state-active-data]
- **WHEN** validation inspects `EngineState`
- **THEN** every field is owned by or actively used by the engine turn reducer
- **THEN** no `CoreState` field exists unless a tested composition path reduces or interprets it as part of a documented engine input/effect contract

#### Scenario: adapter composition is explicit
r[embeddable-agent-engine.explicit-adapter-composition]
- **WHEN** Clankers needs to combine prompt lifecycle policy with turn execution policy
- **THEN** adapter code explicitly sequences `clankers-core` inputs/effects and `clankers-engine` inputs/effects
- **THEN** the composition seam is testable without daemon protocol, TUI rendering, provider I/O, or tool execution

### Requirement: Boundary rails MUST enforce reducer ownership

The repository MUST provide validation rails that catch core/engine ownership drift.
r[embeddable-agent-engine.core-engine-boundary-rails]

#### Scenario: source rails reject cross-reducer policy leakage
r[embeddable-agent-engine.cross-reducer-source-rail]
- **WHEN** validation inventories non-test `clankers-core`, `clankers-engine`, controller core-effect adapters, and agent turn adapters
- **THEN** it fails if core-owned lifecycle policy is implemented inside `clankers-engine`
- **THEN** it fails if engine-owned turn policy is reintroduced as authoritative branching in controller or agent shells

#### Scenario: composition tests cover positive and negative sequencing
r[embeddable-agent-engine.composition-tests]
- **WHEN** validation runs adapter composition tests
- **THEN** positive tests cover prompt start/completion followed by engine turn execution and post-prompt follow-up evaluation
- **THEN** negative tests cover out-of-order completion, mismatched effect IDs, wrong-phase engine feedback, and attempted lifecycle/turn feedback to the wrong reducer
