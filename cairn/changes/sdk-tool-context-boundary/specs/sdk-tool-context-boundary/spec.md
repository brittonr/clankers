## ADDED Requirements

### Requirement: Legacy tool context is compatibility-only [r[sdk-tool-context-boundary.legacy-context]]

`clankers-agent::ToolContext` MUST be treated as a compatibility adapter for existing built-in tools, not as the reusable SDK tool execution contract.

#### Scenario: context service users are inventoried [r[sdk-tool-context-boundary.inventory]]
- GIVEN a tool consumes DB, search, hooks, progress/events, session identity, capability, or cancellation through `ToolContext`
- WHEN the migration inventory runs
- THEN the tool MUST be listed with the concrete service family it consumes
- AND the inventory MUST name the neutral service replacement or compatibility reason

#### Scenario: new reusable tools avoid legacy service fields [r[sdk-tool-context-boundary.legacy-context.compatibility-only]]
- GIVEN new reusable tool behavior is added
- WHEN it needs host storage, search, hooks, progress, capability, or cancellation
- THEN it MUST use neutral `clankers-tool-host` service traits or DTOs
- AND it MUST NOT add new concrete service fields to `ToolContext`

### Requirement: Built-in tools migrate to neutral services [r[sdk-tool-context-boundary.neutral-services]]

Representative built-in tool paths MUST consume host services through `ToolHostServices` or equivalent neutral contracts before the legacy context drain is considered active.

#### Scenario: representative tools use neutral services [r[sdk-tool-context-boundary.neutral-services.representative-tools]]
- GIVEN a representative storage/search path and a hook/progress path are selected
- WHEN those tools execute
- THEN storage/search, hook decisions, progress, capability, and cancellation behavior MUST flow through neutral service DTOs
- AND the old legacy runner MUST only bridge unmigrated compatibility tools

#### Scenario: missing services fail closed [r[sdk-tool-context-boundary.neutral-services.missing-service]]
- GIVEN a migrated tool requires a neutral host service
- WHEN the service is not injected
- THEN the tool MUST return a typed safe error or denial receipt
- AND it MUST NOT open desktop databases, discover globals, or bypass policy

### Requirement: Tool context drain is verified [r[sdk-tool-context-boundary.verification]]

Verification MUST combine migrated-tool fixtures, legacy parity tests, and source-boundary rails.

#### Scenario: fixtures cover service behavior [r[sdk-tool-context-boundary.verification.fixtures]]
- GIVEN migrated tools are tested
- WHEN success, missing service, progress, hook/capability denial, and cancellation cases run
- THEN each case MUST assert neutral outcomes and safe metadata

#### Scenario: boundary rail rejects regressions [r[sdk-tool-context-boundary.verification.boundary-rail]]
- GIVEN reusable tool-host code or a migrated neutral tool imports concrete DB, hooks, TUI, daemon protocol, or root tool state
- WHEN validation runs
- THEN the rail MUST fail with the offending path, service family, replacement owner, and requirement id
