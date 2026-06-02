## ADDED Requirements

### Requirement: Tool execution services are neutral [r[neutral-tool-service-context.service-contracts]]

Reusable tool execution MUST receive storage, search, hook, progress, capability, cancellation, and runtime policy access through neutral service traits or DTOs rather than concrete Clankers desktop types.

#### Scenario: Context has no concrete shell service fields [r[neutral-tool-service-context.service-contracts.no-shell-fields]]
- GIVEN the neutral tool invocation context and service bundle are inspected
- WHEN services for storage, search, hooks, progress, capabilities, or cancellation are exposed
- THEN they MUST be typed neutral services or semantic DTOs
- AND they MUST NOT require `clankers_db::Db`, search-index concrete types, `clankers_hooks::HookPipeline`, `AgentEvent`, TUI DTOs, daemon frames, or root-shell state

#### Scenario: Missing services fail closed [r[neutral-tool-service-context.service-contracts.missing-service]]
- GIVEN a tool asks for a host service that was not injected
- WHEN the neutral context handles the request
- THEN it MUST return a typed unsupported/denied result or safe error receipt
- AND it MUST NOT construct desktop defaults, open databases, or silently bypass policy

### Requirement: Controller tool port uses neutral services [r[neutral-tool-service-context.controller-tool-port]]

`ControllerToolPort` MUST invoke tools through a neutral service bundle and keep concrete desktop services at adapter construction edges.

#### Scenario: Concrete fields are edge-owned [r[neutral-tool-service-context.controller-tool-port.edge-owned]]
- GIVEN `ControllerToolPort` executes tool calls
- WHEN source-boundary rails inspect its fields and call path
- THEN concrete DB, hook, progress, and capability dependencies MUST be converted to neutral services before reusable execution
- AND any remaining concrete compatibility field MUST have an owner receipt and convergence condition

### Requirement: Representative tool paths dogfood the context [r[neutral-tool-service-context.representative-migration]]

At least one storage/search path and one hook/progress path MUST execute through neutral services before the migration is considered useful.

#### Scenario: Storage/search path uses neutral service [r[neutral-tool-service-context.representative-migration.storage]]
- GIVEN a representative tool needs storage or search
- WHEN tests invoke the tool through the neutral executor
- THEN it MUST obtain storage/search behavior from the neutral service interface
- AND it MUST handle missing service with a safe typed failure

#### Scenario: Hook/progress path uses neutral service [r[neutral-tool-service-context.representative-migration.hook-progress]]
- GIVEN a representative tool triggers hook or progress behavior
- WHEN tests invoke the tool through the neutral executor
- THEN hook decisions and progress events MUST flow through neutral service DTOs
- AND the test MUST cover continue, denial, and visible progress metadata

### Requirement: Neutral service verification is deterministic [r[neutral-tool-service-context.verification]]

Verification MUST combine neutral service fixtures, legacy adapter parity, representative migrated tool tests, and source-boundary rails.

#### Scenario: Fixtures cover service decisions [r[neutral-tool-service-context.verification.fixtures]]
- GIVEN deterministic fixtures run
- WHEN success, missing service, hook continue/modify/deny, capability denial, cancellation, progress, and legacy adapter paths execute
- THEN each case MUST assert the neutral outcome and safe emitted metadata

#### Scenario: Boundary rail rejects shell imports [r[neutral-tool-service-context.verification.boundary-rail]]
- GIVEN reusable tool-host or runtime neutral context modules import concrete shell systems
- WHEN architecture validation runs
- THEN validation MUST fail with the offending module, service owner, replacement path, and requirement id
