## Requirements

### Requirement: Clankers MUST provide a reusable embeddable engine crate

The system MUST provide a workspace crate named `clankers-engine` that exposes a host-facing agent harness API above `clankers-core` and below Clankers-specific shells such as daemon, TUI, interactive mode, and system-prompt assembly.

#### Scenario: engine crate defines a host-first boundary

- **WHEN** another Rust project depends on `clankers-engine`
- **THEN** it can drive agent execution through engine-owned state, inputs, effects, and outcomes without depending on daemon protocol, TUI state, or interactive mode modules
- **THEN** the public engine boundary uses engine-native plain-data types rather than `DaemonEvent`, `SessionCommand`, or other Clankers app protocol types

#### Scenario: engine crate is layered above clankers-core

- **WHEN** `clankers-engine` evaluates deterministic orchestration policy
- **THEN** it reuses `clankers-core` for pure FCIS-compatible transitions where possible
- **THEN** any additional engine-owned policy that is not yet in `clankers-core` is structured so it can migrate downward into the core rather than back into app shells

### Requirement: The engine API MUST expose explicit host-driven execution contracts

The engine MUST define explicit host-facing contracts for user inputs, model execution requests, tool execution requests, host feedback, and semantic engine events.

#### Scenario: host submits a user prompt through engine input

- **WHEN** a host wants to start or continue a conversation turn
- **THEN** it does so by sending an engine input that carries the user prompt payload and any explicit attachment metadata
- **THEN** the engine returns an engine outcome containing the next state and any model, tool, or semantic event effects to execute

#### Scenario: model execution is requested through explicit engine effects

- **WHEN** the engine decides that model completion work is needed
- **THEN** it emits an explicit model-request effect containing the correlated request identity and the model request payload
- **THEN** the host returns the model completion or model failure through a correlated engine input rather than the engine performing provider I/O directly

#### Scenario: tool execution is requested through explicit engine effects

- **WHEN** the engine decides that tool execution work is needed
- **THEN** it emits an explicit tool-execution effect containing the correlated call identity, tool name, and structured tool input payload
- **THEN** the host returns the tool result or tool failure through a correlated engine input rather than the engine invoking tools directly

#### Scenario: semantic engine events stay separate from transport events

- **WHEN** the engine needs to surface busy-state changes, loop-state changes, user-visible notices, or tool/model lifecycle events
- **THEN** it emits engine-native semantic events
- **THEN** Clankers-specific adapters translate those events into `DaemonEvent`, TUI messages, or other runtime-specific outputs outside the engine boundary

### Requirement: Turn orchestration MUST be engine-owned reusable policy

The reusable engine boundary MUST own the end-to-end turn orchestration policy needed for an embedded agent harness.

#### Scenario: prompt to model to tool to continuation flow is engine-owned

- **WHEN** a turn includes prompt submission, model completion, one or more tool calls, tool results, and a follow-up model continuation
- **THEN** the engine owns the state machine that decides each next action in that sequence
- **THEN** controller, daemon, TUI, and interactive shells only translate host inputs and execute engine-requested effects

#### Scenario: stop reasons and continuation decisions are engine-owned

- **WHEN** a model completion stops normally, requests tools, hits a token limit, or fails
- **THEN** the engine owns the policy for whether the turn stops, retries, schedules tool execution, or emits a failure outcome
- **THEN** app shells do not keep a second authoritative copy of those continuation rules

#### Scenario: tool-result ingestion is engine-owned

- **WHEN** the host reports one or more tool results back to the engine
- **THEN** the engine decides how those results update conversation state and whether another model request is needed
- **THEN** app shells do not re-derive tool continuation policy locally

### Requirement: Message evolution policy MUST be reusable through the engine

The engine MUST own the reusable conversation/message evolution rules needed for an embedded harness.

#### Scenario: engine appends canonical conversation entries

- **WHEN** the host submits user input, the engine receives model output, or the host returns tool results
- **THEN** the engine updates canonical conversation state using engine-owned message evolution rules
- **THEN** embedders do not need to reconstruct Clankers-specific assistant, tool-result, or continuation ordering themselves

#### Scenario: message policy remains independent of prompt assembly

- **WHEN** a host supplies its own system prompt or prompt-building policy
- **THEN** the engine still preserves the same message evolution and turn-orchestration contracts
- **THEN** system-prompt discovery from AGENTS.md, OpenSpec context, skills, or project files remains outside the engine boundary

### Requirement: Controller and agent shells MUST become adapters over the engine

Clankers-specific controller and agent shells MUST consume the engine as an adapter layer rather than remaining the authoritative source of reusable harness semantics.

#### Scenario: controller stops owning reusable orchestration policy

- **WHEN** the controller handles prompt, model, tool, retry, or continuation flows that are in scope for the embeddable engine
- **THEN** it translates between session/daemon shell concerns and engine inputs/effects
- **THEN** it does not remain the only authoritative implementation of those reusable orchestration rules

#### Scenario: agent runtime stops owning reusable turn policy

- **WHEN** the agent runtime performs provider calls, tool execution, hook dispatch, or event streaming
- **THEN** it acts as a host/runtime adapter around engine requests and outcomes
- **THEN** reusable turn policy lives in the engine boundary instead of inside async runtime code in `clankers-agent`

### Requirement: App-specific concerns MUST stay outside the engine boundary

The embeddable engine MUST not absorb Clankers application concerns that are not required for a minimal host-facing harness.

#### Scenario: system prompt assembly stays app-specific

- **WHEN** Clankers loads AGENTS.md, SYSTEM.md, APPEND_SYSTEM.md, OpenSpec context, or skill definitions
- **THEN** that behavior remains outside the engine boundary
- **THEN** the engine accepts already-prepared prompt inputs or explicit prompt-policy configuration rather than reading project context files directly

#### Scenario: transport and UI concerns stay app-specific

- **WHEN** Clankers sends daemon protocol frames, renders TUI state, manages attach-mode overlays, or maintains interactive-mode UI state
- **THEN** those behaviors remain in transport and UI adapters
- **THEN** the engine boundary stays free of protocol framing, TUI widget, and terminal event-loop types

### Requirement: Embedding-focused migration rails MUST verify the target architecture

The system MUST add explicit verification rails that keep future extraction work aligned with the embeddable engine target.

#### Scenario: engine API rails reject app-protocol leakage

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it checks the public `clankers-engine` surface for leakage of app-specific protocol or UI types such as `DaemonEvent`, `SessionCommand`, or TUI widget/runtime types
- **THEN** failure blocks acceptance of the capability work

#### Scenario: engine parity rails cover host-adapter seams

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it includes parity tests proving that controller and agent adapters execute engine-requested model/tool work and feed correlated host results back without re-deriving reusable turn policy locally
- **THEN** failure blocks acceptance of the capability work

#### Scenario: engine turn-state-machine rails cover positive and negative paths

- **WHEN** validation runs for the embeddable engine capability
- **THEN** it includes positive and negative tests for prompt submission, model completion, tool-request planning, tool-result ingestion, retry decisions, cancellation, token-limit handling, and terminal stop behavior
- **THEN** the tests assert deterministic state/effect outcomes for the migrated reusable turn-orchestration slice
