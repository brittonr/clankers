# Embeddable Agent Engine Specification

## Purpose

Defines the `embeddable-agent-engine` capability.

## Requirements

### Requirement: Engine facade exposes neutral DTOs [r[embeddable-agent-engine.neutral-engine-facade]]

The embeddable agent engine MUST expose a reusable Rust facade whose request, event, outcome, and receipt DTOs are neutral to Clankers product shells. Engine callers MUST NOT need to construct CLI command structs, TUI application state, daemon protocol frames, Matrix bridge types, or root-crate mode internals to execute a turn.

#### Scenario: external host constructs a turn request [r[embeddable-agent-engine.neutral-engine-facade.external-host-request]]
- GIVEN an external Rust host wants to run one agent turn
- WHEN it constructs the engine request
- THEN the request MUST use engine-owned DTOs or shared neutral message DTOs
- THEN it MUST NOT require TUI, daemon, attach, Matrix, or root-mode types

#### Scenario: host observes neutral events and receipts [r[embeddable-agent-engine.neutral-engine-facade.neutral-output]]
- GIVEN an engine turn emits progress, model output, tool output, usage, or completion information
- WHEN the host observes the output
- THEN the output MUST be represented as neutral engine events, outcomes, or receipts
- THEN display and protocol adapters MAY project those outputs to TUI or daemon surfaces outside the engine

### Requirement: Host supplies effect ports [r[embeddable-agent-engine.host-supplied-effect-ports]]

The embeddable agent engine MUST express model completion, tool execution, persistence/history, prompt loading, hook execution, skills/context lookup, and cost accounting through explicit ports or adapter-owned DTO boundaries. The engine MUST NOT construct live provider/auth/router state, open shell-specific storage, or run app-edge hooks directly when a host adapter should own that effect.

#### Scenario: model and tool effects are replaceable [r[embeddable-agent-engine.host-supplied-effect-ports.replaceable-model-tool]]
- GIVEN a deterministic fixture host runs the engine
- WHEN model completion or tool execution is requested
- THEN fake model and tool ports MUST be usable without live credentials or backend processes
- THEN the engine MUST record neutral outcomes rather than backend-specific policy details

#### Scenario: shell adapters own app-edge effects [r[embeddable-agent-engine.host-supplied-effect-ports.shell-adapter-effects]]
- GIVEN the Clankers product shell wires the engine
- WHEN provider auth, router construction, session file access, hook execution, or TUI rendering is needed
- THEN shell adapters MUST own those concrete effects
- THEN reusable engine modules MUST receive only ports, neutral DTOs, or already-loaded inputs

### Requirement: Clankers shells dogfood the engine API [r[embeddable-agent-engine.shell-dogfoods-engine-api]]

Standalone, controller, daemon, TUI, and attach paths SHOULD converge on the same engine facade that external hosts use. A migration slice MAY adapt one path at a time, but it MUST preserve existing behavior with focused parity checks and MUST NOT create a second independent turn orchestrator.

#### Scenario: one shell path delegates to the facade [r[embeddable-agent-engine.shell-dogfoods-engine-api.one-path-delegates]]
- GIVEN an existing Clankers shell path runs an agent turn
- WHEN the first migration slice lands
- THEN at least one shell path MUST delegate turn orchestration through the engine facade or a facade-compatible adapter
- THEN focused parity checks MUST show the migrated path preserves its previous observable behavior

#### Scenario: no parallel turn engine is introduced [r[embeddable-agent-engine.shell-dogfoods-engine-api.no-parallel-engine]]
- GIVEN an implementation adds the embeddable engine facade
- WHEN existing turn logic remains during migration
- THEN shared orchestration logic MUST be reused or delegated rather than copied into a competing engine
- THEN any temporary compatibility path MUST name its removal or convergence path in tasks or comments

### Requirement: Fixture host proves embeddability [r[embeddable-agent-engine.fixture-host-proves-embeddability]]

The embeddable agent engine MUST include a deterministic fixture host or test harness that runs at least one minimal turn with fake ports and no live credentials, sockets, daemon, TUI, Matrix, or root CLI dependencies. The fixture output MUST be stable enough to serve as a regression contract for external embedding.

#### Scenario: positive fixture turn runs without product shell [r[embeddable-agent-engine.fixture-host-proves-embeddability.positive-turn]]
- GIVEN the fixture host provides fake model, tool, persistence, hook, prompt, and accounting ports
- WHEN it runs a minimal turn through the engine facade
- THEN the turn MUST complete without live credentials, sockets, daemon state, TUI state, Matrix state, or root CLI mode construction
- THEN the emitted events or receipt MUST be deterministic under repeated runs

#### Scenario: missing required port fails closed [r[embeddable-agent-engine.fixture-host-proves-embeddability.missing-port]]
- GIVEN the engine requires a host-supplied effect for a turn
- WHEN the host omits or denies that effect
- THEN the engine MUST return a typed failure outcome or receipt
- THEN it MUST NOT silently construct a live product-shell fallback

### Requirement: Engine modules reject inward display and protocol leaks [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks]]

Reusable engine modules MUST NOT import root shell mode modules, TUI application/view/rendering types, daemon protocol frame types, Matrix bridge types, attach transport types, or concrete provider/auth/router construction types directly. Concrete adapters MAY import both engine and shell types, but the ownership boundary MUST remain explicit.

#### Scenario: architecture rail detects forbidden imports [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks.forbidden-imports]]
- GIVEN a reusable engine module is checked by architecture rails
- WHEN it imports or constructs forbidden shell/display/protocol/concrete-provider types
- THEN the rail MUST fail with the offending module, forbidden dependency, and requirement ID
- THEN the fix MUST move that dependency behind an adapter or host port

#### Scenario: adapter modules are explicit exceptions [r[embeddable-agent-engine.no-inward-display-or-protocol-leaks.adapter-exceptions]]
- GIVEN an adapter projects engine DTOs to a shell, TUI, daemon, Matrix, attach, or provider surface
- WHEN architecture rails inspect the adapter
- THEN the adapter MAY import both sides if it is named and owned as an edge adapter
- THEN reusable engine core modules MUST remain free of those imports

### Requirement: Engine architecture rails are deterministic [r[embeddable-agent-engine.engine-architecture-rails]]

Verification for the embeddable engine boundary MUST use deterministic checks such as Cargo metadata, Rust AST inspection, fixture receipts, compile-fail/negative fixtures, or typed manifests. Rails MUST report actionable owner diagnostics instead of relying only on unstructured grep folklore.

#### Scenario: rail output names the violated owner [r[embeddable-agent-engine.engine-architecture-rails.owner-diagnostics]]
- GIVEN a forbidden dependency or duplicated orchestration path appears
- WHEN the engine architecture rail runs
- THEN it MUST report the source module, target dependency or duplicated owner, and violated requirement ID
- THEN the diagnostic MUST be deterministic across repeated runs

#### Scenario: fixture receipts are stable [r[embeddable-agent-engine.engine-architecture-rails.stable-fixtures]]
- GIVEN the fixture host runs the same minimal turn twice
- WHEN receipts or event summaries are compared
- THEN stable fields MUST match deterministically
- THEN volatile host paths, credentials, live provider data, and wall-clock-only values MUST NOT be required for the comparison
