# Steel Default Orchestration Basalt Contract Bridge Delta

## ADDED Requirements

### Requirement: Basalt contract bridge for Steel turn planning [r[steel-default-orchestration.basalt-contract-bridge]]

Clankers MUST bind the real `steel.host.plan_turn` path to Basalt's Steel contract DTO boundary by constructing and validating Basalt Steel evaluation requests before claiming Steel turn-planning output is contract-backed. Clankers MUST remain the runtime owner for Steel evaluation, fallback decisions, and host-effect authorization.

#### Scenario: Basalt request is constructed from safe planning metadata [r[steel-default-orchestration.basalt-contract-bridge.request]]
- GIVEN Steel turn planning is configured for `steel.host.plan_turn`
- WHEN Clankers prepares a Steel planning evaluation
- THEN it MUST construct a Basalt Steel evaluation request for the selected seam
- AND the request MUST include only safe metadata, hashes, schemas, required UCAN/session capabilities, evaluator identity, and bounded input descriptors
- AND it MUST NOT include raw prompts, provider payloads, credentials, tokens, connection strings, or unbounded script output

#### Scenario: Basalt request validation gates contract-backed evaluation [r[steel-default-orchestration.basalt-contract-bridge.validation]]
- GIVEN a Basalt Steel evaluation request has been constructed
- WHEN Clankers is about to treat Steel output as contract-backed
- THEN it MUST validate the request with Basalt's public validator
- AND invalid, unsupported, malformed, or under-authorized requests MUST fail closed before any host effect is authorized

#### Scenario: Basalt receipt evidence is hash-bound and redacted [r[steel-default-orchestration.basalt-contract-bridge.receipts]]
- GIVEN Steel turn planning evaluates through the Basalt contract bridge
- WHEN Clankers emits an orchestration receipt
- THEN the receipt MUST include Basalt request schema, request hash, receipt schema, receipt hash or invalid-receipt reason, and safe evaluator/backend metadata
- AND it MUST omit raw prompts, provider payloads, scripts, secrets, credentials, tokens, and connection strings

#### Scenario: Clankers keeps runtime and host-effect authority [r[steel-default-orchestration.basalt-contract-bridge.runtime-ownership]]
- GIVEN Basalt validates the contract DTO boundary
- WHEN Steel returns a typed plan or request for host action
- THEN Clankers MUST still use its Steel runtime wrapper, Rust fallback/block policy, and dynamic-runtime authorization seam before any effect
- AND Basalt validation MUST NOT grant ambient filesystem, process, git, network, provider, daemon, TUI, or credential authority

#### Scenario: Bridge failures fail closed according to Clankers policy [r[steel-default-orchestration.basalt-contract-bridge.fail-closed]]
- GIVEN Basalt request validation, receipt validation, UCAN ability checks, session capability checks, or schema checks fail
- WHEN the turn-planning decision is evaluated
- THEN Clankers MUST either use the configured Rust-native fallback or block the planning decision
- AND it MUST emit a stable issue code and safe summary
- AND it MUST NOT execute a host effect from the failed Steel plan

#### Scenario: Agent turn path emits Basalt-bound evidence [r[steel-default-orchestration.basalt-contract-bridge.agent-turn]]
- GIVEN Steel turn planning is enabled from reviewed settings/profile material
- WHEN a real agent turn reaches the `steel.host.plan_turn` adapter
- THEN the emitted turn-planning receipt MUST carry Basalt-bound request/receipt evidence
- AND repeated identical inputs MUST produce stable Basalt request/receipt hash fields

#### Scenario: External Basalt fixture remains green [r[steel-default-orchestration.basalt-contract-bridge.external-fixture]]
- GIVEN Clankers wires the product path to Basalt's DTO surface
- WHEN the downstream Basalt consumer fixture is tested
- THEN the fixture MUST still compile and pass against the sibling Basalt checkout

#### Scenario: Lifecycle closeout is verified [r[steel-default-orchestration.basalt-contract-bridge.closeout]]
- GIVEN the bridge implementation and tests are complete
- WHEN the change is closed
- THEN Cairn validation, proposal/design/tasks gates, focused Rust tests, the Basalt consumer fixture, sync/archive, and diff checks MUST pass before commit
