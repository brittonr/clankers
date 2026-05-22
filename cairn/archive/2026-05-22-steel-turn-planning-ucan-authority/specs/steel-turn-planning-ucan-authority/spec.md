# Steel Turn Planning UCAN Authority Specification

## Purpose

Defines the `steel-turn-planning-ucan-authority` capability for authorizing reviewed Steel Scheme `steel.host.plan_turn` activation with explicit UCAN-backed runtime authority while preserving Rust-owned enforcement and receipts.

## Requirements

### Requirement: Stable UCAN vocabulary for Steel turn planning [r[steel-turn-planning-ucan-authority.vocabulary]]

Clankers MUST map Steel turn-planning authority to a stable planning-specific UCAN ability and normalized resource before any UCAN decision is evaluated.

#### Scenario: planning seam maps to stable ability and resource [r[steel-turn-planning-ucan-authority.vocabulary.plan-turn]]
- GIVEN reviewed settings select the `steel.host.plan_turn` seam
- WHEN Rust builds authority facts for the turn planner
- THEN the requested ability MUST be the Steel planning ability, not a provider, tool, filesystem, shell, network, or mutation ability
- AND the requested resource MUST be normalized from the session/profile/script context with only safe display material

#### Scenario: unknown planning seam fails closed [r[steel-turn-planning-ucan-authority.vocabulary.unknown-seam]]
- GIVEN settings or profile data request a Steel host seam other than the reviewed `steel.host.plan_turn` seam
- WHEN Rust builds authority facts
- THEN activation MUST fail before UCAN authorization or Steel execution
- AND the denial MUST identify the unmapped seam without exposing secrets

### Requirement: Rust-owned adapter authorizes Steel planning invocation [r[steel-turn-planning-ucan-authority.adapter]]

Clankers MUST evaluate a UCAN invocation decision through a narrow Rust-owned adapter before Steel turn planning can run for an enabled config.

#### Scenario: matching UCAN grant allows reviewed planner [r[steel-turn-planning-ucan-authority.adapter.allowed]]
- GIVEN settings/profile/script validation succeeds for `steel.host.plan_turn`
- AND the UCAN adapter returns an allowed invocation decision for the normalized planning ability, resource, caveats, audience, and proof references
- WHEN a real agent turn is constructed
- THEN Rust MAY invoke the existing Steel turn-planning adapter
- AND provider/tool/session effects still require Rust-owned execution paths after the typed plan is parsed

#### Scenario: denied UCAN blocks before Steel and provider execution [r[steel-turn-planning-ucan-authority.adapter.denied]]
- GIVEN the UCAN proof is missing, expired, revoked, replayed, wrong-audience, wrong-ability, wrong-resource, overbroad, malformed, unavailable, or denied by caveat policy
- WHEN a real agent turn attempts Steel turn-planning activation
- THEN activation MUST fail closed before Steel script execution
- AND no provider call, tool call, filesystem access, process execution, network request, daemon mutation, TUI mutation, credential access, or native-tool call may be required to hide or recover from the denial

#### Scenario: UCAN adapter seam avoids local reimplementation [r[steel-turn-planning-ucan-authority.adapter.public-ucan-api]]
- GIVEN Clankers evaluates Steel turn-planning authority
- WHEN it verifies proof chains, replay, revocation, expiry, audience, attenuation, or caveats
- THEN it MUST call a narrow UCAN adapter backed by public UCAN-library APIs where available
- AND it MUST NOT locally reimplement raw compact token parsing, proof traversal, signing-key handling, revocation, replay, or attenuation semantics

### Requirement: Evaluation order preserves existing config safety [r[steel-turn-planning-ucan-authority.evaluation-order]]

UCAN authorization MUST be added after existing profile/script/hash/budget validation and before any Steel planning execution.

#### Scenario: disabled config remains disabled without authority lookup [r[steel-turn-planning-ucan-authority.evaluation-order.disabled]]
- GIVEN no Steel turn-planning config is present or the config is disabled
- WHEN Rust builds a real turn config
- THEN no Steel planner is configured
- AND Clankers MUST NOT emit a receipt claiming UCAN-authorized Steel planning occurred

#### Scenario: invalid reviewed artifacts fail before authority lookup [r[steel-turn-planning-ucan-authority.evaluation-order.invalid-artifact]]
- GIVEN profile or script validation fails because of missing paths, malformed data, unsupported seam, hash mismatch, unsupported host action, or budget violation
- WHEN Rust evaluates activation
- THEN activation MUST fail before Steel execution
- AND UCAN authorization MUST NOT be used to bypass or repair invalid reviewed artifacts

#### Scenario: normal and orchestrated turns use the same authority helper [r[steel-turn-planning-ucan-authority.evaluation-order.shared-helper]]
- GIVEN normal and orchestrated/model-role phase turns can construct Steel turn-planning configuration
- WHEN UCAN authority evaluation is added
- THEN both call sites MUST use the same Rust-owned helper or adapter path
- AND their allow/deny/fallback semantics MUST NOT drift

### Requirement: UCAN cannot grant Steel ambient authority [r[steel-turn-planning-ucan-authority.no-ambient-authority]]

A UCAN grant for Steel turn planning MUST authorize only invocation of the reviewed planning seam. It MUST NOT grant Steel ambient filesystem, shell, git, network, provider, credential, daemon, TUI, native-tool, session-mutation, code-mutation, or tool-execution authority.

#### Scenario: planning grant cannot widen host functions [r[steel-turn-planning-ucan-authority.no-ambient-authority.host-functions]]
- GIVEN a UCAN grant allows Steel turn planning
- WHEN Steel code is evaluated
- THEN only Rust-registered planning host functions for the reviewed seam may be available
- AND route/script/profile changes MUST NOT add new host functions or widen runtime permissions

#### Scenario: action requests still pass Rust authorization [r[steel-turn-planning-ucan-authority.no-ambient-authority.action-requests]]
- GIVEN Steel returns a typed plan or dynamic-runtime action envelope
- WHEN Rust interprets the plan
- THEN any requested provider, tool, filesystem, shell, network, daemon, TUI, credential, mutation, or Wasm action MUST pass its own Rust/Nickel/UCAN/session/disabled-action checks
- AND the planning grant alone MUST be insufficient authority for those effects

### Requirement: Authority receipts are deterministic and redacted [r[steel-turn-planning-ucan-authority.receipts]]

Clankers MUST record safe deterministic Steel planning authority receipts for allowed and denied decisions without persisting raw secrets or prompt/script/profile bodies.

#### Scenario: allowed receipt records safe proof metadata [r[steel-turn-planning-ucan-authority.receipts.allowed]]
- GIVEN UCAN authorization allows Steel turn planning
- WHEN Rust records the planning authority receipt
- THEN the receipt MUST include the seam, planning ability, normalized or redacted resource reference, authorization status, profile hash, script hash, safe issuer/audience/proof reference, caveat classes, replay/revocation status where applicable, and a receipt hash
- AND it MUST exclude raw compact UCAN tokens, signing material, headers, environment values, prompts, provider payloads, profile bodies, and script bodies

#### Scenario: denied receipt redacts denial details [r[steel-turn-planning-ucan-authority.receipts.redacted]]
- GIVEN UCAN authorization denies or cannot evaluate Steel turn planning
- WHEN Rust records the denial
- THEN the receipt MUST include a structured denial class and safe proof/reference metadata
- AND raw tokens, signing keys, secret caveat values, raw prompts, raw scripts, raw profile bodies, and provider payloads MUST NOT be printed, persisted, or exposed through daemon/session events

### Requirement: Implementation has focused verification evidence [r[steel-turn-planning-ucan-authority.verification]]

The implementation MUST include focused tests, docs, and a deterministic checker receipt proving allowed planning authority, fail-closed denial behavior, no ambient authority, shared turn-path activation, and receipt redaction.

#### Scenario: focused tests cover authority decisions [r[steel-turn-planning-ucan-authority.verification.tests]]
- GIVEN valid and invalid UCAN authority fixtures for Steel turn planning
- WHEN focused tests run
- THEN they MUST prove matching grants allow the reviewed planner
- AND they MUST prove missing, expired, revoked, wrong-audience, wrong-resource, wrong-ability, unknown-caveat, and overbroad grants deny before Steel/provider/tool execution

#### Scenario: checker writes redacted authority receipt [r[steel-turn-planning-ucan-authority.verification.checker]]
- GIVEN the source tree includes the authority implementation, docs, and Cairn tasks
- WHEN the checker runs
- THEN it MUST write a deterministic receipt under `target/steel-turn-planning-ucan-authority/`
- AND the receipt MUST hash reviewed artifacts without embedding raw compact UCAN tokens, signing material, prompts, provider payloads, raw scripts, raw profile bodies, or absolute secret paths

### Requirement: Change is archived onto clean main [r[steel-turn-planning-ucan-authority.archive]]

Clankers MUST sync/archive the Cairn change after implementation verification and land it on clean pushed `main`.

#### Scenario: accepted spec remains durable after archive [r[steel-turn-planning-ucan-authority.archive.durable-spec]]
- GIVEN implementation verification passes
- WHEN the Cairn change is synced and archived
- THEN the accepted spec MUST retain the authority requirements
- AND `main` MUST be pushed with a clean working tree
