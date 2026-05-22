# Lego Architecture Boundaries Specification

## Purpose

Defines the `lego-architecture-boundaries` capability.

## Requirements

### Requirement: Root crate remains a thin product shell [r[lego-architecture-boundaries.root-shell-thinness]]

The root `clankers` crate MUST act as a CLI/product shell that parses arguments, initializes app-edge services, wires reusable bricks, and dispatches commands. It MUST NOT own reusable domain policy, provider request shaping, process-job policy, storage schemas, rendering semantics, or transport protocol conversion inline when those concerns can live behind workspace-crate APIs.

#### Scenario: root wiring is allowed but domain policy is not [r[lego-architecture-boundaries.root-shell-thinness.wiring-only]]
- GIVEN a feature requires multiple workspace bricks
- WHEN the root crate handles the feature
- THEN root code MAY construct configuration, choose a command handler, and pass dependencies into a brick
- THEN reusable policy, DTO validation, request shaping, storage mutation, and rendering rules MUST live in named workspace modules or crates with focused tests

#### Scenario: root dependency growth has an ownership receipt [r[lego-architecture-boundaries.root-shell-thinness.dependency-receipt]]
- GIVEN a change adds a new internal dependency to the root crate
- WHEN the architecture rail runs
- THEN it MUST report the dependency owner and reason
- THEN the change MUST either prove the dependency is app-edge wiring or move the reusable behavior into an existing brick

### Requirement: Agent uses ports instead of concrete systems [r[lego-architecture-boundaries.agent-uses-ports-not-concrete-systems]]

`clankers-agent` MUST orchestrate turns through explicit ports/traits or narrow DTO adapters for provider calls, tool execution, config reads, storage, hooks, prompts, skills, cost tracking, and runtime services. The agent MUST NOT directly encode concrete provider/router/storage/TUI/runtime policies that can be tested as independent bricks.

#### Scenario: provider calls flow through a model port [r[lego-architecture-boundaries.agent-uses-ports-not-concrete-systems.model-port]]
- GIVEN the turn loop needs model output
- WHEN the agent builds and executes the request
- THEN provider-native request shaping and routing decisions MUST be delegated to a model port implementation
- THEN turn-level tests MUST be able to substitute a deterministic fake without constructing live provider/router/auth state

#### Scenario: tool execution flows through a tool port [r[lego-architecture-boundaries.agent-uses-ports-not-concrete-systems.tool-port]]
- GIVEN the turn loop needs a tool call executed
- WHEN the agent dispatches the call
- THEN capability gating, backend execution, output truncation, and receipt projection MUST be delegated through a tool-execution port
- THEN the agent MUST record neutral tool outcomes rather than owning backend-specific policy inline

### Requirement: Controller seams are single-purpose [r[lego-architecture-boundaries.controller-seams-are-single-purpose]]

`clankers-controller` MUST keep input translation, core effect interpretation, continuation policy, event translation, session persistence, and transport projection in separately testable modules. No controller module SHOULD simultaneously own command parsing, effect execution, daemon/TUI projection, and follow-up policy.

#### Scenario: command input translation is isolated [r[lego-architecture-boundaries.controller-seams-are-single-purpose.input-translation]]
- GIVEN a daemon, local, or attach command arrives
- WHEN the controller maps it to core input
- THEN the mapping MUST be testable without running an agent turn, opening sockets, or constructing TUI state
- THEN transport-specific framing MUST remain outside the input translation module

#### Scenario: event projection is isolated from core effect policy [r[lego-architecture-boundaries.controller-seams-are-single-purpose.event-projection]]
- GIVEN the controller receives agent or core events
- WHEN events are projected to daemon protocol or TUI surfaces
- THEN projection MUST occur through explicit adapters
- THEN core effect policy MUST NOT depend on daemon/TUI event constructors

### Requirement: Process tool is a thin adapter over process-job services [r[lego-architecture-boundaries.process-tool-thin-adapter]]

The agent-visible `process` tool MUST parse tool JSON into typed request DTOs, call a process-job service, and project typed receipts/errors back to tool output. It MUST NOT own backend dispatch, durable storage mapping, notification policy, retention/GC policy, redaction policy, or backend capability rules inline.

#### Scenario: tool adapter does not import storage DTOs directly [r[lego-architecture-boundaries.process-tool-thin-adapter.no-storage-dto-imports]]
- GIVEN the process tool handles any action
- WHEN the architecture rail inspects the tool adapter module
- THEN the adapter MUST NOT construct or mutate persisted database DTOs directly
- THEN persistence-owned DTO conversion MUST live in the process-job service or persistence adapter

#### Scenario: request and receipt fixtures prove adapter thinness [r[lego-architecture-boundaries.process-tool-thin-adapter.fixture-proof]]
- GIVEN representative start, poll, log, wait, stdin, and kill requests
- WHEN fixtures exercise the tool adapter
- THEN assertions MUST verify typed request DTOs and typed receipt projections
- THEN at least one negative fixture MUST fail closed before backend dispatch for unsupported action or policy denial

### Requirement: Provider/router has one owner per concern [r[lego-architecture-boundaries.provider-router-has-one-owner-per-concern]]

Provider-native request-body construction, auth/account probing, routing/fallback/cooldown, retry/refresh behavior, and stream normalization MUST each have one explicit owner. Compatibility adapters MAY translate between public Clankers DTOs and that owner, but MUST NOT duplicate the same policy in parallel layers.

#### Scenario: request shaping has a single implementation owner [r[lego-architecture-boundaries.provider-router-has-one-owner-per-concern.request-shaping]]
- GIVEN a routed provider request is sent
- WHEN the request body is built
- THEN exactly one provider-specific module owns the native JSON/body shape
- THEN tests MUST compare against literal fixtures rather than building expected JSON by calling the implementation under test

#### Scenario: routing policy is not duplicated in provider compatibility code [r[lego-architecture-boundaries.provider-router-has-one-owner-per-concern.routing-policy]]
- GIVEN model selection uses routing, fallback, cooldown, or provider availability
- WHEN compatibility adapters are present
- THEN adapters MUST delegate routing policy to the router owner
- THEN adapters MUST NOT silently implement independent fallback/cooldown behavior for the same provider family

### Requirement: Display and protocol types do not leak inward [r[lego-architecture-boundaries.display-and-protocol-types-do-not-leak-inward]]

Agent, runtime, and core controller logic MUST emit neutral domain events, command outcomes, and receipts. TUI, daemon protocol, Matrix, and remote attach code MUST project those neutral DTOs through explicit adapters. Display/protocol DTO crates MUST NOT become the canonical domain model for agent/runtime decisions.

#### Scenario: TUI DTOs stay at display edges [r[lego-architecture-boundaries.display-and-protocol-types-do-not-leak-inward.tui-edge]]
- GIVEN agent or runtime logic emits a message, tool result, usage update, or process receipt
- WHEN the event reaches the TUI
- THEN it MUST pass through a domain-to-TUI projection adapter
- THEN agent/runtime modules MUST NOT need to import TUI-only constructors for decision-making

#### Scenario: daemon protocol DTOs stay at transport edges [r[lego-architecture-boundaries.display-and-protocol-types-do-not-leak-inward.daemon-edge]]
- GIVEN a daemon or remote attach client observes session activity
- WHEN protocol frames are produced
- THEN they MUST be projected from neutral domain events or receipts
- THEN transport framing code MUST NOT reimplement agent/runtime policy to synthesize behavior-specific events

### Requirement: Attach parity uses shared policy core [r[lego-architecture-boundaries.attach-parity-uses-shared-policy-core]]

Standalone, daemon, local attach, and remote attach command paths MUST share one session command/effect/ack policy core for thinking level changes, disabled tools, compaction, queued prompts, prompt lifecycle, and daemon acknowledgement suppression. Transport-specific code MAY deliver commands and events, but MUST NOT reimplement parity policy independently.

#### Scenario: slash command effect is shared across transports [r[lego-architecture-boundaries.attach-parity-uses-shared-policy-core.command-effect]]
- GIVEN a supported session command is issued in standalone, daemon, local attach, or remote attach mode
- WHEN the command is applied
- THEN all paths MUST call the same policy core to determine local state changes, daemon commands, expected acknowledgements, and user-visible messages
- THEN parity fixtures MUST cover at least one positive and one no-op/fail-closed command path across transports

#### Scenario: reconnect clears only transport-local suppression state [r[lego-architecture-boundaries.attach-parity-uses-shared-policy-core.reconnect]]
- GIVEN a remote attach client reconnects
- WHEN pending daemon events are drained
- THEN transport-local suppression budgets MUST reset
- THEN durable session policy state MUST remain owned by the shared policy core or controller state, not by stale attach-local trackers

### Requirement: Typed architecture rails replace brittle string folklore [r[lego-architecture-boundaries.typed-architecture-rails]]

Architecture boundary verification MUST prefer typed Cargo metadata, Rust AST checks, deterministic fixtures, generated manifests, or receipt validation over unstructured string-presence checks. Temporary string rails MAY remain only when they name an owner, failure mode, and replacement path.

#### Scenario: dependency boundary rail reports owners [r[lego-architecture-boundaries.typed-architecture-rails.dependency-owners]]
- GIVEN an internal crate dependency graph is checked
- WHEN a forbidden or newly coupled edge appears
- THEN the rail MUST report source crate, target crate, allowed owner if any, and the boundary requirement that failed
- THEN the diagnostic MUST not require manual grep archaeology to find the owner

#### Scenario: source boundary rail parses code structure [r[lego-architecture-boundaries.typed-architecture-rails.ast-boundaries]]
- GIVEN a boundary forbids provider calls in an agent adapter or storage DTOs in a tool adapter
- WHEN the rail checks source code
- THEN it SHOULD use AST/module/import analysis or a typed manifest where practical
- THEN any remaining string-token check MUST be documented as a temporary guard with a follow-up owner
