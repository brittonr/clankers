## ADDED Requirements

### Requirement: Steel eval agent tool request contract [r[steel-eval-agent-tool.request-contract]]

Clankers MUST expose the agent-visible `steel_eval` tool as a typed built-in tool that delegates all Steel execution to the existing Clankers Steel runtime wrapper. The tool request MUST accept bounded Steel source plus an optional reviewed runtime profile identifier and MUST reject malformed, oversized, or unsupported requests before evaluation.

#### Scenario: Tool delegates to the runtime wrapper [r[steel-eval-agent-tool.request-contract.wrapper-delegation]]
- GIVEN the `steel_eval` tool receives a syntactically valid bounded request
- WHEN evaluation is attempted
- THEN the tool MUST construct the Clankers-owned Steel runtime request DTO
- AND it MUST call the existing Steel runtime wrapper rather than constructing Steel interpreter internals directly

#### Scenario: Profile selection is explicit [r[steel-eval-agent-tool.request-contract.profile-selection]]
- GIVEN a `steel_eval` request names a runtime profile
- WHEN the tool validates the request
- THEN the profile MUST be present in reviewed settings or policy material
- AND missing, disabled, malformed, or unsupported profiles MUST fail closed before evaluation

### Requirement: Steel eval tool registration policy [r[steel-eval-agent-tool.registration-policy]]

Clankers MUST register `steel_eval` only when reviewed settings/profile material enables agent exposure and the runtime wrapper reports the selected profile as available. Tool discovery, daemon `ToolList`, and disabled-tool rebuild behavior MUST remain consistent across standalone, daemon, local attach, and remote attach contexts.

#### Scenario: Enabled profile exposes the tool [r[steel-eval-agent-tool.registration-policy.enabled]]
- GIVEN reviewed settings/profile material enables `steel_eval`
- WHEN Clankers builds the available tool list
- THEN the tool list MUST include `steel_eval` with its request schema and safe description
- AND the description MUST identify it as a constrained embedded interpreter, not an OS/process/VM sandbox

#### Scenario: Unavailable runtime fails closed [r[steel-eval-agent-tool.registration-policy.unavailable]]
- GIVEN Steel support, the runtime wrapper, or the selected profile is unavailable
- WHEN Clankers builds or invokes the `steel_eval` tool
- THEN the tool MUST be omitted from discovery or return a typed unavailable denial according to the reviewed registration policy
- AND it MUST NOT fall back to any other interpreter, shell, provider, process, network, filesystem, daemon, TUI, credential, or native tool authority

#### Scenario: Tool-list parity is preserved [r[steel-eval-agent-tool.registration-policy.tool-list-parity]]
- GIVEN standalone, daemon, local attach, or remote attach sessions use the same enabled Steel tool profile
- WHEN tool discovery or daemon `ToolList` events are produced
- THEN each context MUST expose the same `steel_eval` availability state and request schema
- AND attach replay or reconnect MUST NOT duplicate, hide, or stale the tool entry

#### Scenario: Disabled-tool parity is preserved [r[steel-eval-agent-tool.registration-policy.disabled-parity]]
- GIVEN a user disables `steel_eval` through the normal disabled-tool mechanism
- WHEN a model invokes the tool or tool lists are rebuilt
- THEN Clankers MUST deny or hide `steel_eval` consistently with other built-in tools
- AND the denial receipt MUST use the same disabled-tool policy path as standalone and daemon contexts

### Requirement: Steel eval authority boundary [r[steel-eval-agent-tool.authority-boundary]]

The default `steel_eval` tool profile MUST provide pure bounded Steel evaluation with zero ambient host authority. Any host function exposed to Steel MUST be explicitly registered by the selected profile and checked against session capabilities, disabled-tool policy, and runtime budgets before execution.

#### Scenario: Pure default has no host authority [r[steel-eval-agent-tool.authority-boundary.pure-default]]
- GIVEN `steel_eval` runs under the default profile
- WHEN the Steel source attempts filesystem, process, network, provider/router, credential, daemon/session, TUI, environment, clock, git, mutation, or native tool access
- THEN Clankers MUST deny the attempt with a stable issue code
- AND the forbidden effect MUST NOT execute

#### Scenario: Unknown host function is denied [r[steel-eval-agent-tool.authority-boundary.denied-host-function]]
- GIVEN Steel source calls an unknown, disabled, unsupported, over-budget, or unauthorized host function
- WHEN `steel_eval` evaluates the source
- THEN the runtime MUST return a typed denial outcome
- AND the tool MUST NOT retry through a broader profile or alternate host authority

#### Scenario: No ambient fallback authority exists [r[steel-eval-agent-tool.authority-boundary.no-ambient-fallback]]
- GIVEN `steel_eval` fails request validation, profile validation, evaluation, host-function authorization, or budget enforcement
- WHEN the tool reports the failure
- THEN it MUST NOT execute filesystem, process, shell, network, provider, credential, daemon, TUI, git, mutation, or native tool fallback work
- AND it MUST report only the safe failure class and stable issue code

### Requirement: Steel eval receipt contract [r[steel-eval-agent-tool.receipt-contract]]

Every `steel_eval` invocation MUST emit deterministic, bounded, redacted receipt data that records the tool schema, profile id, runtime outcome, source hash, output hash/length, host-call summary, issue codes, and redaction class without leaking raw secrets, credentials, provider payloads, raw prompts, connection strings, unbounded scripts, or oversized output.

#### Scenario: Success receipt is deterministic [r[steel-eval-agent-tool.receipt-contract.success]]
- GIVEN identical Steel source, profile material, runtime wrapper version, and allowed host functions
- WHEN `steel_eval` succeeds repeatedly
- THEN the receipt MUST contain stable schema/version, profile, source hash, output hash/length, host-call summary, and success outcome fields
- AND deterministic fields MUST match across repeated runs

#### Scenario: Failure receipt is typed [r[steel-eval-agent-tool.receipt-contract.failure]]
- GIVEN `steel_eval` fails due to unavailable runtime, invalid request, invalid profile, parse/eval error, denied host function, disabled tool, or resource limit
- WHEN the tool returns the failure
- THEN the receipt MUST include a stable issue code and safe failure class
- AND it MUST NOT include raw interpreter diagnostics that contain source, paths, credentials, provider payloads, or secrets

#### Scenario: Redaction protects sensitive material [r[steel-eval-agent-tool.receipt-contract.redaction]]
- GIVEN Steel source or output contains secret-like strings, credentials, paths, provider payload markers, raw prompts, or oversized content
- WHEN `steel_eval` constructs user-visible output and receipts
- THEN configured redaction and output limits MUST bound the user-visible material
- AND receipts MUST retain hashes/lengths/classes rather than leaking the sensitive or oversized content

### Requirement: Steel eval verification contract [r[steel-eval-agent-tool.verification-contract]]

The implementation MUST include deterministic positive and negative tests or fixtures for request validation, runtime-wrapper delegation, pure evaluation, disabled/unavailable tool behavior, denied host functions, resource limits, redaction, receipt stability, and standalone/daemon/attach tool-list parity.

#### Scenario: Positive fixture proves pure eval [r[steel-eval-agent-tool.verification-contract.positive-fixture]]
- GIVEN a deterministic pure Steel expression fixture
- WHEN `steel_eval` runs it twice under the same profile
- THEN both runs MUST succeed through the runtime wrapper
- AND stable receipt fields MUST match

#### Scenario: Negative fixture proves denial [r[steel-eval-agent-tool.verification-contract.negative-fixture]]
- GIVEN fixtures for disabled tool, unavailable profile, unknown host function, and over-budget evaluation
- WHEN each fixture invokes `steel_eval`
- THEN each MUST fail closed with the expected stable issue code
- AND no forbidden host effect may execute

#### Scenario: Redaction fixture proves receipt safety [r[steel-eval-agent-tool.verification-contract.redaction-fixture]]
- GIVEN fixture source and output containing secret-like and oversized material
- WHEN `steel_eval` returns output and receipts
- THEN tests MUST prove the sensitive material is omitted or bounded according to policy
- AND receipt hashes/lengths/classes remain deterministic

#### Scenario: Parity fixture proves discovery consistency [r[steel-eval-agent-tool.verification-contract.parity-fixture]]
- GIVEN the same reviewed Steel tool profile is used in standalone, daemon, local attach, and remote attach tool discovery paths
- WHEN discovery, disabled-tool rebuild, replay, or reconnect behavior is exercised
- THEN `steel_eval` availability and disabled-tool behavior MUST remain consistent across those contexts

#### Scenario: Lifecycle closeout is verified [r[steel-eval-agent-tool.verification-contract.closeout]]
- GIVEN the `steel_eval` implementation and fixtures are complete
- WHEN the change is closed
- THEN focused Rust tests, Cairn validation, proposal/design/tasks gates, sync/archive inspection, and diff checks MUST pass before commit
