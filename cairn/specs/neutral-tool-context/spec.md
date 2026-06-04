# Neutral Tool Context Specification

## Purpose

Defines the `neutral-tool-context` capability.

## Requirements

### Requirement: Tool invocation context is shell-neutral [r[neutral-tool-context.context-contract]]

Reusable tool execution APIs MUST expose a neutral invocation context rather than Clankers agent, TUI, database, or hook internals.

#### Scenario: Context fields are neutral [r[neutral-tool-context.context-contract.neutral-fields]]
- GIVEN a reusable tool executor receives an invocation context
- WHEN public fields and required constructor arguments are inspected
- THEN the context MAY include call id, cancellation, safe progress/event sinks, capability metadata, and typed host service handles
- AND it MUST NOT require `AgentEvent`, `clankers_db::Db`, search-index concrete types, hook-pipeline internals, TUI progress DTOs, daemon frames, or root-shell state

### Requirement: Old and new tool APIs bridge explicitly [r[neutral-tool-context.adapter-compatibility]]

Existing agent tools MAY be supported during migration only through named adapters between shell `Tool` implementations and neutral `ToolExecutor` contracts.

#### Scenario: Bridge owns compatibility [r[neutral-tool-context.adapter-compatibility.old-new-bridge]]
- GIVEN a legacy built-in or plugin tool still implements the agent `Tool` trait
- WHEN the engine-host path invokes it
- THEN a named adapter MUST translate neutral context/outcome data to the legacy shell shape
- AND reusable tool-host modules MUST NOT import the legacy agent trait directly

### Requirement: Tool host services hide concrete shell systems [r[neutral-tool-context.host-services]]

Persistence, search, hooks, process management, and progress display required by tools MUST be accessed through host-provided services or semantic DTOs.

#### Scenario: No shell-only fields in neutral context [r[neutral-tool-context.host-services.no-shell-fields]]
- GIVEN a tool needs storage, search, hooks, process monitoring, or progress streaming
- WHEN the neutral context exposes that capability
- THEN it MUST expose a typed service trait or semantic DTO
- AND absent services MUST fail closed or report unsupported behavior without constructing Clankers desktop defaults

### Requirement: Representative tools migrate first [r[neutral-tool-context.migration]]

At least one read-only built-in and one mutating or progress-emitting path MUST dogfood the neutral context before the migration is considered useful.

#### Scenario: Representative tools use neutral context [r[neutral-tool-context.migration.representative-tools]]
- GIVEN representative built-in tools are migrated
- WHEN tests invoke them through `ToolExecutor`
- THEN they MUST run without direct `AgentEvent`, `Db`, TUI widget, daemon, or root-shell construction

### Requirement: Neutral tool verification is deterministic [r[neutral-tool-context.verification]]

The tool-context migration MUST include behavioral fixtures and source-boundary rails.

#### Scenario: Tool fixtures cover positive and negative paths [r[neutral-tool-context.verification.tool-fixtures]]
- GIVEN deterministic tool fixtures run
- WHEN success, missing storage, capability denial, cancellation, progress, and truncation cases execute
- THEN each case MUST assert the neutral outcome and safe emitted metadata

#### Scenario: Boundary rail catches shell imports [r[neutral-tool-context.verification.boundary-rail]]
- GIVEN reusable tool-host context modules are inspected
- WHEN they import forbidden shell-only dependencies
- THEN validation MUST fail with the offending module and violated requirement id

### Requirement: Supported tool service ports are dogfooded first [r[neutral-tool-context.supported-service-ports]]

A tool-host service/context API MUST NOT be promoted from experimental to supported until deterministic fixtures exercise positive and fail-closed behavior through the public API.

#### Scenario: promoted service port has positive and negative fixtures [r[neutral-tool-context.supported-service-ports.fixtures]]
- GIVEN a storage, search, hook, progress, capability, cancellation, or runtime-policy service API is promoted
- WHEN validation runs
- THEN fixtures MUST exercise the service through `ToolInvocationContext` or an equivalent neutral public API
- AND absent or denied service behavior MUST fail closed without constructing desktop defaults

#### Scenario: docs match promoted service semantics [r[neutral-tool-context.supported-service-ports.docs]]
- GIVEN a service/context API is classified as supported
- WHEN SDK docs are checked
- THEN the docs MUST describe host responsibilities, positive behavior, fail-closed behavior, and app-edge boundaries for that API
