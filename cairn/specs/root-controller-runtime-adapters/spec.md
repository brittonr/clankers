# Root Controller Runtime Adapters Specification

## Purpose

Defines the `root-controller-runtime-adapters` capability.

## Requirements

### Requirement: Root crate is composition only [r[root-controller-runtime-adapters.root-shell]]

The root `clankers` crate MUST act as a product shell for CLI parsing, desktop service construction, mode dispatch, and adapter wiring. Reusable domain policy MUST live in workspace bricks or named modules with focused tests.

#### Scenario: Root code does not own reusable policy [r[root-controller-runtime-adapters.root-shell.composition-only]]
- GIVEN a root module handles a feature
- WHEN architecture validation inspects that module
- THEN it MAY parse CLI, initialize services, choose a mode, and wire adapters
- AND request shaping, session persistence policy, tool execution policy, provider routing policy, semantic event policy, and rendering rules MUST be delegated to named bricks or edge adapters

### Requirement: Controller is a runtime/session shell [r[root-controller-runtime-adapters.controller-shell]]

`clankers-controller` MUST keep command lifecycle and transport-agnostic orchestration separate from concrete provider, database, config, protocol, TUI, and session storage implementations.

#### Scenario: Controller uses service interfaces [r[root-controller-runtime-adapters.controller-shell.service-interfaces]]
- GIVEN controller code submits prompts, applies controls, projects events, or manages session identity
- WHEN reusable behavior is required
- THEN the controller MUST use runtime/session service interfaces or semantic event adapters
- AND concrete provider/db/config/protocol/TUI construction MUST remain outside the reusable command policy path

#### Scenario: Fake service path proves separation [r[root-controller-runtime-adapters.controller-shell.fake-service-path]]
- GIVEN fake runtime/session services are supplied
- WHEN a controller prompt/control fixture runs
- THEN it MUST exercise command lifecycle and event projection without opening sockets, constructing TUI state, initializing providers, or touching desktop session storage

### Requirement: Root/controller dependency budgets are receipted [r[root-controller-runtime-adapters.dependency-budget]]

Root and controller internal dependency growth MUST be tracked with owner receipts.

#### Scenario: Dependency receipts name owners [r[root-controller-runtime-adapters.dependency-budget.owner-receipts]]
- GIVEN architecture validation inventories root and controller dependencies
- WHEN concrete internal dependencies are present or added
- THEN the rail MUST report source crate, target crate, owner category, adapter module, and convergence condition
- AND unowned reusable-policy dependencies MUST fail validation

### Requirement: Adapter shell verification is deterministic [r[root-controller-runtime-adapters.verification]]

Verification MUST prove fake-service controller behavior and existing shell parity.

#### Scenario: Controller fixtures cover command lifecycle [r[root-controller-runtime-adapters.verification.controller-fixtures]]
- GIVEN fake runtime/session services are used
- WHEN prompt submission, cancellation, thinking level, disabled tools, session identity, and semantic event projection are exercised
- THEN results MUST be deterministic and independent of sockets, TUI state, providers, and desktop storage

#### Scenario: Closeout preserves daemon/attach parity [r[root-controller-runtime-adapters.verification.closeout]]
- GIVEN implementation is complete
- WHEN focused validation runs
- THEN daemon/attach parity fixtures, FCIS boundary rail, dependency ownership rail, Cairn validation/gates, and diff checks MUST pass
