# Steel Default Orchestration Specification

## Purpose

Defines the `steel-default-orchestration` capability for making Steel Scheme the reviewed default orchestration/planning seam for selected Clankers agent decisions while preserving Rust-owned enforcement and host-effect seams.

## Requirements

### Requirement: Steel orchestration is policy selected [r[steel-default-orchestration.policy-selected-default]]

Clankers MUST enable Steel default orchestration only through a reviewed orchestration profile/policy that names the planning seam, script source, script hash requirement, runtime budget profile, fallback mode, allowed host actions, receipt requirements, and rollout stage.

#### Scenario: default profile names the seam [r[steel-default-orchestration.policy-selected-default.named-seam]]
- GIVEN a profile enables Steel orchestration by default
- WHEN Clankers starts a supported planning decision
- THEN the selected profile MUST name the exact planning seam
- AND it MUST NOT apply Steel orchestration globally to unrelated decisions

#### Scenario: disabled profile uses Rust-native planner [r[steel-default-orchestration.policy-selected-default.disabled]]
- GIVEN policy disables Steel orchestration
- WHEN the same planning decision occurs
- THEN Clankers MUST use the Rust-native planner
- AND it MUST emit no claim that Steel authored the decision

### Requirement: Rust owns the orchestration adapter seam [r[steel-default-orchestration.rust-adapter-seam]]

Clankers MUST route Steel orchestration through a Rust-owned adapter that implements the same planner interface as the Rust-native planner. Agent, controller, daemon, TUI, attach, provider, and tool-host shells MUST NOT construct Steel interpreter internals directly.

#### Scenario: callers depend on planner interface [r[steel-default-orchestration.rust-adapter-seam.interface]]
- GIVEN a caller needs an orchestration decision
- WHEN Steel orchestration is available
- THEN the caller MUST invoke the Rust planner interface or adapter
- AND it MUST NOT branch on interpreter-specific APIs

#### Scenario: wrapper owns evaluation [r[steel-default-orchestration.rust-adapter-seam.wrapper-only]]
- GIVEN the Steel planner evaluates a script
- WHEN evaluation occurs
- THEN it MUST call the existing Clankers Steel runtime wrapper
- AND it MUST preserve wrapper-owned profile, budget, host-function, and receipt behavior

### Requirement: Steel returns typed plans only [r[steel-default-orchestration.typed-plan-output]]

Steel orchestration MUST return typed versioned plans or dynamic-runtime action envelopes. Free-form textual script output MUST NOT be executable authority.

#### Scenario: typed plan is accepted for review [r[steel-default-orchestration.typed-plan-output.accepted]]
- GIVEN a Steel script returns a supported plan schema
- WHEN Rust parses the plan
- THEN Rust MAY continue to policy and authority checks
- AND the receipt MUST include the plan schema, script hash, profile, and redaction class

#### Scenario: malformed plan falls back or blocks [r[steel-default-orchestration.typed-plan-output.malformed]]
- GIVEN a Steel script returns malformed or unsupported output
- WHEN Rust parses the plan
- THEN Rust MUST reject the plan with a stable issue code
- AND Rust MAY use the Rust-native fallback only when policy allows fallback

### Requirement: Host effects remain Rust authorized [r[steel-default-orchestration.rust-authorized-effects]]

Every effectful action requested by a Steel plan MUST cross Rust authorization before execution, including Nickel policy checks, UCAN/session capability checks, disabled-tool checks, runtime profile checks, provider/router ownership, and mutation preflight/apply/rollback seams where applicable.

#### Scenario: allowed action envelope crosses existing seam [r[steel-default-orchestration.rust-authorized-effects.allowed-envelope]]
- GIVEN Steel returns a dynamic-runtime action envelope allowed by policy
- WHEN Rust evaluates the plan
- THEN Rust MUST authorize it through the existing dynamic-runtime authorization seam
- AND the Steel script MUST NOT execute the effect directly

#### Scenario: denied host action performs no fallback effect [r[steel-default-orchestration.rust-authorized-effects.denied-envelope]]
- GIVEN Steel returns an unknown, disabled, unauthorized, or over-budget host action
- WHEN Rust evaluates the plan
- THEN Rust MUST deny the action before any host effect
- AND it MUST NOT retry through filesystem, process, git, network, provider, credential, daemon, TUI, or native-tool fallback authority

### Requirement: Fallback is explicit and receipt-backed [r[steel-default-orchestration.fallback-and-receipts]]

Steel orchestration failure MUST produce deterministic receipts and use Rust-native fallback only when policy explicitly allows it. Fallback MUST NOT loosen the Steel runtime profile or silently hide repeated Steel failures.

#### Scenario: script failure emits fallback receipt [r[steel-default-orchestration.fallback-and-receipts.script-failure]]
- GIVEN Steel script load, evaluation, or typed-plan parsing fails
- WHEN fallback is allowed
- THEN Clankers MUST emit a receipt recording the Steel failure class, script/profile identity, fallback policy, and Rust-native fallback decision class
- AND it MUST NOT include raw secrets, credentials, provider payloads, or unbounded script output

#### Scenario: fallback disabled blocks safely [r[steel-default-orchestration.fallback-and-receipts.fallback-disabled]]
- GIVEN Steel script load, evaluation, or typed-plan parsing fails
- WHEN fallback is disabled
- THEN Clankers MUST block the orchestration decision with a stable issue code
- AND no host effect may execute from the failed Steel plan

### Requirement: Rollout evidence precedes default expansion [r[steel-default-orchestration.rollout-evidence]]

Before Steel becomes default for additional planning seams, Clankers MUST collect comparison evidence between Steel planner output and Rust-native planner output for the reviewed seam, including plan hashes, decision class, authorized effect summary, denial summary, and fallback status.

#### Scenario: comparison receipt is stable [r[steel-default-orchestration.rollout-evidence.comparison-receipt]]
- GIVEN Steel orchestration is enabled in comparison mode
- WHEN a planning decision is evaluated
- THEN Clankers MUST emit a deterministic comparison receipt
- AND the receipt MUST be stable across repeated runs with identical inputs and policies

#### Scenario: expansion requires reviewed profile update [r[steel-default-orchestration.rollout-evidence.reviewed-expansion]]
- GIVEN a new planning seam is proposed for Steel default orchestration
- WHEN the profile is updated
- THEN the update MUST include reviewed policy, fixtures, fallback behavior, and receipt evidence for that seam
- AND it MUST NOT inherit authority from another seam implicitly

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
