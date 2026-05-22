# Polyglot Agent Architecture Specification

## Purpose

Defines Clankers' cross-layer agent-kit architecture for composing Nickel, Rust, Steel Scheme, Wasm, and UCAN without blurring configuration, orchestration, untrusted tool execution, and host authority.

## Requirements

### Requirement: Architecture MUST define a stable division of labor [r[polyglot-agent-architecture.division-of-labor]]

Clankers MUST document and enforce a stable division of labor where Nickel owns declarative contracts, Rust owns engine/I/O/enforcement/receipts, Steel Scheme owns trusted orchestration over typed host functions, Wasm owns untrusted or third-party tool execution behind capability imports, and UCAN owns runtime delegated authority.

#### Scenario: layer ownership is reviewable [r[polyglot-agent-architecture.division-of-labor.reviewable-layers]]
- GIVEN an agent feature uses Nickel, Rust, Steel, Wasm, or UCAN
- WHEN reviewers inspect the feature's design or implementation
- THEN the feature MUST state which layer owns configuration, orchestration, execution, authority, and receipts
- AND it MUST NOT require reviewers to infer authority boundaries from ad hoc runtime behavior

#### Scenario: Steel and Wasm do not compete for the same authority [r[polyglot-agent-architecture.division-of-labor.steel-wasm-complementary]]
- GIVEN an agent workflow includes both trusted orchestration and untrusted tool execution
- WHEN Clankers routes the workflow
- THEN Steel Scheme MAY decide or request the next typed action through host functions
- AND Wasm MAY execute capability-limited tool code
- AND Rust MUST remain the enforcement point for both paths

### Requirement: Nickel MUST validate agent persona, prompt, model, and tool contracts before use [r[polyglot-agent-architecture.nickel-agent-contracts]]

Agent identity, prompt templates, model/profile choices, runtime budgets, tool manifests, and JSON schemas MUST be representable as Nickel-authored contracts exported into typed Rust-consumable data before an agent profile boots or is activated.

#### Scenario: prompt template variables are checked before boot [r[polyglot-agent-architecture.nickel-agent-contracts.prompt-template-validation]]
- GIVEN an agent profile references prompt template variables
- WHEN the Nickel export/check rail evaluates the profile
- THEN missing required variables, duplicate names, malformed schema entries, and unsupported model/profile fields MUST be rejected before runtime activation
- AND the failure MUST use stable issue codes without embedding credentials or provider payloads

#### Scenario: tool schema matches host registration [r[polyglot-agent-architecture.nickel-agent-contracts.tool-schema-host-parity]]
- GIVEN a Nickel-authored tool manifest declares a tool name and JSON schema
- WHEN Rust loads the exported agent profile
- THEN every declared tool MUST match a registered host tool, plugin tool, or disabled explicit placeholder
- AND runtime startup MUST fail closed when the exported schema and host registration disagree

### Requirement: Rust MUST remain the authority and receipt layer [r[polyglot-agent-architecture.rust-authority]]

Rust host code MUST own provider I/O, memory/session persistence, filesystem/process/network/credential/daemon/TUI authority, UCAN verification, policy loading, deterministic receipts, verification, and rollback. Dynamic runtimes MAY request actions but MUST NOT bypass Rust authority.

#### Scenario: dynamic runtime request crosses a typed host-function seam [r[polyglot-agent-architecture.rust-authority.typed-host-function-seam]]
- GIVEN Steel or Wasm requests an action with host-visible effects
- WHEN the request reaches Rust
- THEN Rust MUST validate the typed DTO, Nickel-derived policy, UCAN authority when required, disabled-tool/session capability state, and runtime profile before the effect executes
- AND the dynamic runtime MUST receive only a structured result or denial receipt

#### Scenario: receipts are emitted by the host [r[polyglot-agent-architecture.rust-authority.host-owned-receipts]]
- GIVEN a dynamic runtime action succeeds, fails, or is denied
- WHEN Clankers reports the outcome
- THEN Rust MUST emit the deterministic receipt
- AND the receipt MUST use hashes, stable status codes, and safe metadata rather than raw prompts, credentials, compact UCAN tokens, provider payloads, or oversized tool bodies

### Requirement: Steel Scheme MUST be trusted orchestration, not ambient authority [r[polyglot-agent-architecture.steel-orchestration]]

Steel Scheme MAY define hot-reloadable reasoning loops, routing logic, scoring, planning, and requests for host actions, but it MUST NOT receive ambient filesystem, process, git, network, provider, credential, daemon, TUI, or native-tool authority.

#### Scenario: hot-reloaded orchestration preserves host boundaries [r[polyglot-agent-architecture.steel-orchestration.hot-reload-boundary]]
- GIVEN an operator hot-reloads a Steel orchestration script
- WHEN the script runs under an agent profile
- THEN it MAY alter workflow decisions that are expressible through the approved host-function surface
- AND it MUST NOT gain new host functions, broader capabilities, larger budgets, or mutation authority without an explicit profile/policy/UCAN change

#### Scenario: Steel sandbox overclaims are rejected [r[polyglot-agent-architecture.steel-orchestration.no-sandbox-overclaim]]
- GIVEN Steel support is documented, surfaced in status output, or described in receipts
- WHEN the implementation lacks OS-level isolation proof
- THEN Clankers MUST call Steel a constrained embedded interpreter or trusted orchestration runtime
- AND it MUST NOT claim Steel provides VM/process/OS-level sandbox isolation

### Requirement: Wasm MUST execute untrusted tools through capability-limited imports [r[polyglot-agent-architecture.wasm-tool-sandbox]]

Wasm plugin and generated-code execution MUST run with explicit imports, bounded memory/fuel/time budgets, host-provided inputs, manifest-declared schemas, and no ambient filesystem or network authority unless an import is explicitly granted and recorded.

#### Scenario: generated code runs ephemerally [r[polyglot-agent-architecture.wasm-tool-sandbox.ephemeral-generated-code]]
- GIVEN an agent asks to execute generated or untrusted code
- WHEN Clankers accepts the request
- THEN Rust MUST create a bounded Wasm execution context with only declared imports
- AND it MUST collect structured output and destroy or recycle the context without leaking ambient host authority

#### Scenario: Wasm sandbox language is precise [r[polyglot-agent-architecture.wasm-tool-sandbox.no-magic-sandbox-claim]]
- GIVEN documentation or user-facing status describes Wasm tool execution
- WHEN no host-runtime proof covers a stronger claim
- THEN Clankers MUST describe safety in terms of denied imports, capability limits, budgets, and runtime tests
- AND it MUST NOT claim escape is mathematically impossible as a product guarantee

### Requirement: UCAN MUST distinguish runtime delegation from declarative policy [r[polyglot-agent-architecture.ucan-runtime-authority]]

Sensitive actions MUST require a matching runtime UCAN grant in addition to any Nickel policy allowance. UCAN validation MUST check ability, normalized resource, audience/session binding, expiry, revocation, and delegation limits where applicable.

#### Scenario: policy-allowed action without UCAN is denied [r[polyglot-agent-architecture.ucan-runtime-authority.policy-not-enough]]
- GIVEN Nickel policy allows an action class in principle
- WHEN a Steel script, Wasm tool, or engine workflow requests a sensitive action without a matching UCAN grant
- THEN Rust MUST deny the action before side effects
- AND the receipt MUST report safe denial metadata without raw proof material

#### Scenario: matching UCAN does not bypass Nickel policy [r[polyglot-agent-architecture.ucan-runtime-authority.ucan-not-enough]]
- GIVEN a runtime UCAN grant names an ability and resource
- WHEN Nickel policy disallows the target class, profile, path, tool, or verb
- THEN Rust MUST deny the action before side effects
- AND the receipt MUST identify the policy denial class using safe metadata

### Requirement: Verification rails MUST prevent boundary drift [r[polyglot-agent-architecture.verification-rails]]

Implementation MUST include deterministic checks that prevent layer-boundary drift, validate Nickel exported profiles, prove Steel host-function denial, prove Wasm capability limits, and verify safe receipts for allowed and denied dynamic-runtime actions.

#### Scenario: dependency rail blocks interpreter/runtime leakage [r[polyglot-agent-architecture.verification-rails.dependency-boundary]]
- GIVEN the workspace is checked by architecture rails
- WHEN a generic engine/core/tool schema crate directly imports Steel interpreter internals, live Nickel evaluation, or Wasm runtime internals outside its approved adapter boundary
- THEN verification MUST fail and identify the offending crate or module

#### Scenario: positive and negative fixtures cover dynamic runtime actions [r[polyglot-agent-architecture.verification-rails.dynamic-runtime-fixtures]]
- GIVEN deterministic fixtures exercise Steel orchestration and Wasm tool execution
- WHEN verification runs
- THEN allowed actions MUST produce stable success receipts
- AND denied actions for missing UCAN, policy mismatch, disabled host function, ambient Steel authority, and missing Wasm import MUST fail closed before forbidden effects
