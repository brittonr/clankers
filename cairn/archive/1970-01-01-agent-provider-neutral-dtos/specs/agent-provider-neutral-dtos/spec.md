## ADDED Requirements

### Requirement: Agent neutral DTOs use neutral owners [r[agent-provider-neutral-dtos.neutral-imports]]

Reusable `clankers-agent` policy MUST import conversation messages, content blocks, stop reasons, usage, tool-result messages, and stream deltas from neutral DTO owners such as `clanker-message` rather than through provider reexports.

#### Scenario: Provider reexports are absent from reusable policy [r[agent-provider-neutral-dtos.neutral-imports.no-provider-reexports]]
- GIVEN non-test agent modules are inspected
- WHEN a module only needs neutral message, usage, or stream DTOs
- THEN it MUST import those DTOs from `clanker-message` or another neutral owner
- AND it MUST NOT import them through `clankers_provider::message`, `clankers_provider::Usage`, or provider streaming reexports

### Requirement: Provider-native execution is adapter-owned [r[agent-provider-neutral-dtos.model-adapter]]

Provider-native request construction, provider trait calls, and provider stream adaptation MUST be confined to named model adapter modules with explicit owner receipts and convergence conditions.

#### Scenario: CompletionRequest is adapter-only [r[agent-provider-neutral-dtos.model-adapter.completion-request]]
- GIVEN `CompletionRequest` or `Provider` appears in `clankers-agent`
- WHEN source-boundary rails inspect non-test code
- THEN the reference MUST be inside an approved model adapter owner or compatibility shim
- AND the rail diagnostic MUST name the neutral model request seam that reusable turn policy should use instead

#### Scenario: Turn policy uses neutral model data [r[agent-provider-neutral-dtos.model-adapter.turn-policy]]
- GIVEN turn policy prepares model execution inputs or consumes model stream outputs
- WHEN the reusable policy path is inspected
- THEN it MUST use engine/runtime/message DTOs and model-port abstractions
- AND provider-native body/request details MUST be absent from the policy path

### Requirement: Runtime provider seam is explicit [r[agent-provider-neutral-dtos.runtime-model-seam]]

The agent model port migration MUST leave a documented neutral seam toward `clankers-runtime` provider/router services even if provider-native execution remains in the desktop adapter for this slice.

#### Scenario: Dependency budget has a smaller convergence condition [r[agent-provider-neutral-dtos.runtime-model-seam.budget]]
- GIVEN the agent still depends on `clankers-provider`
- WHEN dependency ownership inventory is generated
- THEN the receipt MUST identify provider usage as model-adapter-only
- AND it MUST state the next convergence condition for replacing provider-native requests with runtime/engine neutral DTOs

### Requirement: Agent provider drain verification is deterministic [r[agent-provider-neutral-dtos.verification]]

Verification MUST combine import/source rails, focused agent behavior tests, and dependency ownership updates.

#### Scenario: Import rail catches provider DTO drift [r[agent-provider-neutral-dtos.verification.import-rail]]
- GIVEN a future reusable agent module imports neutral DTOs through `clankers-provider`
- WHEN the boundary rail runs
- THEN validation MUST fail with the module, offending import, neutral owner, and requirement id

#### Scenario: Closeout preserves turn behavior [r[agent-provider-neutral-dtos.verification.closeout]]
- GIVEN imports and adapter ownership are migrated
- WHEN closeout validation runs
- THEN focused agent turn/compaction/tool-substrate tests, architecture rails, Cairn gates/validate, and compile checks MUST pass or include explicit checked evidence for environmental limitations
