# embedded-composition-kits Specification

## Purpose

Define reusable embedded SDK composition kits that prove product-owned provider, tool, session, capability, and runtime-dispatch seams can be validated without daemon, TUI, provider discovery, live credentials, or shell-owned state.

## Requirements

### Requirement: Provider adapter fixtures are product owned

The provider-adapter kit SHALL use checked fixtures and product-owned model capability profiles rather than deriving expected contracts from live provider/router implementations.

#### Scenario: provider-adapter-template-is-fixture-backed
- GIVEN the provider adapter kit validates its request and response examples
- WHEN `scripts/check-provider-adapter-kit.rs` runs
- THEN the fixture file MUST define completed, retryable-failure, terminal-failure, and usage-accounting cases.

#### Scenario: model-capability-profile-remains-product-owned
- GIVEN a product-owned adapter describes model capabilities
- WHEN the kit validates the profile
- THEN the profile MUST reject live credentials and network requirements.

#### Scenario: template-dependency-boundary-is-enforced
- GIVEN the provider adapter example is compiled
- WHEN its manifest and source are checked
- THEN it MUST avoid provider discovery, router, OAuth, live-network, and Clankers shell dependencies.

### Requirement: Session/resume brick convergence

The session-resume-brick SHALL demonstrate reusable neutral session-ledger DTOs plus product-owned session/message stores that can restore context across more than one product-shaped store without depending on shell session storage.

#### Scenario: Multiple product-shaped stores prove restored context
- GIVEN multiple product-shaped stores replay a saved session fixture
- WHEN the resume kit reloads the session
- THEN restored context MUST include the expected user, assistant, and tool-result DTOs.

#### Scenario: Missing and stale sessions fail closed
- GIVEN a missing, stale, or schema-incompatible session is requested
- WHEN the resume kit validates the store
- THEN it MUST fail closed before fabricating context.

#### Scenario: Reusable session ledger API is promoted
- GIVEN the resume behavior has converged across product-shaped stores
- WHEN a reusable runtime API is used
- THEN `SessionLedgerEntry` history and `Runtime::resume_session` MUST restore ordered context while missing or unsupported stores fail closed before model/tool execution.

### Requirement: Tool catalog manifests are runtime neutral

The tool-catalog-manifest kit SHALL export normalized, runtime-neutral tool metadata and diagnostics.

#### Scenario: Manifest export is normalized and runtime-neutral
- GIVEN a tool catalog manifest is emitted
- WHEN `scripts/check-tool-catalog-manifest.rs` validates it
- THEN runtime-neutral names, capabilities, approval, and redaction metadata MUST be present without shell runtime identifiers.

#### Scenario: Manifest validation diagnostics are actionable
- GIVEN invalid manifests are checked
- WHEN validation fails
- THEN diagnostics MUST name the bad field and reason.

#### Scenario: Normalized evidence distinguishes semantic drift
- GIVEN manifest evidence is compared between runs
- WHEN only ordering or formatting changes
- THEN semantic drift MUST be distinguished from non-semantic normalization changes.

### Requirement: Capability pack composition is deterministic

Capability pack composition SHALL merge safe packs deterministically and fail closed on dangerous conflicts.

#### Scenario: Pack merge order is deterministic
- GIVEN multiple capability packs are selected
- WHEN composition runs
- THEN the resulting capability order MUST be deterministic.

#### Scenario: Dangerous conflicts fail closed
- GIVEN a pack contains dangerous capabilities
- WHEN it lacks explicit dangerous and approval metadata
- THEN composition MUST deny the pack before Rust use.

#### Scenario: Pack policy is checked before Rust use
- GIVEN a composed pack would be consumed by Rust adapters
- WHEN policy validation fails
- THEN no Rust adapter may execute from that pack.

### Requirement: Plugin runtime dispatch is explicit

Runtime plugin dispatch SHALL distinguish Extism, stdio, built-in, and product-owned runtime kinds before loading or execution.

#### Scenario: Runtime kind dispatch is explicit
- GIVEN a plugin descriptor declares a runtime kind
- WHEN dispatch planning runs
- THEN the kind MUST be selected from the allowed dispatch matrix.

#### Scenario: Launch policy is contract checked
- GIVEN a runtime kind has launch policy metadata
- WHEN validation runs
- THEN unsupported loaders or forbidden launch policies MUST be rejected.

#### Scenario: Dispatch matrix evidence is content addressed
- GIVEN runtime dispatch evidence is emitted
- WHEN the receipt is generated
- THEN the dispatch matrix MUST be content addressed for review and replay.

### Requirement: Experimental SDK ports have an owner budget [r[embedded-composition-kits.experimental-port-budget]]

Every public embedded SDK item labeled `experimental` MUST have a recorded owner, use-site status, and disposition: promote with evidence, keep experimental with rationale, or make private.

#### Scenario: experimental inventory is actionable [r[embedded-composition-kits.experimental-port-budget.actionable]]
- GIVEN the generated SDK inventory contains experimental rows
- WHEN the experimental budget rail runs
- THEN each row MUST be grouped by crate and owner module
- AND each group MUST name the next convergence action and validation path

#### Scenario: unused experimental ports do not remain public by accident [r[embedded-composition-kits.experimental-port-budget.hide-unused]]
- GIVEN an experimental port has no production adapter, fixture, or documented product recipe use
- WHEN the port is reviewed during a resolution slice
- THEN it MUST either gain deterministic evidence or become private/compatibility-scoped
