# Steel Execute Turn Authority Specification

## Purpose

Defines the explicit `steel.host.execute_turn` authority contract for default Steel-selected execution so the planning grant does not implicitly authorize provider/tool host-runner work.

## Requirements

### Requirement: Default profile declares separate execution authority [r[steel-execute-turn-authority.profile]]

The reviewed Steel default orchestration profile MUST declare `steel.host.execute_turn` as a separate host action with its own session capability and UCAN ability.

#### Scenario: profile separates planning and execution grants [r[steel-execute-turn-authority.profile.separate-grants]]
- GIVEN the bundled Steel default orchestration profile is loaded
- WHEN Rust validates allowed host actions
- THEN `steel.host.plan_turn` MUST require planning authority
- AND `steel.host.execute_turn` MUST require `turn-execution` session capability and `clankers/steel/orchestrate.execute_turn` UCAN ability
- AND execution authority MUST NOT be inferred from the planning grant alone

### Requirement: Steel-selected execution authorizes before host runner [r[steel-execute-turn-authority.pre-run]]

When a default Steel plan selects the Steel execution adapter, Rust MUST build a metadata-only execution input DTO and authorize `steel.host.execute_turn` before calling provider/tool host-runner code.

#### Scenario: allowed execution reaches Rust host runner [r[steel-execute-turn-authority.pre-run.allowed]]
- GIVEN the selected plan is authorized for `executor=SteelScheme`
- AND the session has execution session capability plus matching UCAN ability
- WHEN the Steel-selected execution adapter runs
- THEN Rust MUST authorize a `SteelTurnExecutionInput` through dynamic-runtime authorization
- AND only then call the Rust host runner

#### Scenario: denied execution blocks provider work [r[steel-execute-turn-authority.pre-run.denied]]
- GIVEN the selected plan is authorized for `executor=SteelScheme`
- BUT execution session capability, UCAN ability, profile action, budget, receipt destination, or disabled-action policy denies `steel.host.execute_turn`
- WHEN the Steel-selected execution adapter runs
- THEN Rust MUST emit a denied execution receipt
- AND it MUST NOT call the provider/tool host runner

### Requirement: Execution receipts are daemon-visible and redacted [r[steel-execute-turn-authority.receipts]]

Allowed and denied execution authority decisions MUST be visible through existing daemon/session system-message events without exposing raw prompts, provider payloads, tool bodies, credentials, UCAN proofs, or script source.

#### Scenario: allowed receipt records authority facts [r[steel-execute-turn-authority.receipts.allowed]]
- GIVEN execution authority is allowed
- WHEN the host runner returns
- THEN the `steel.host.execute_turn` receipt MUST include execution schema, executor, session hash, model label, result status, host-runner label, authority status/reason, required UCAN ability, required session capability list, input hash, authority receipt hash, safe counts, and receipt hash
- AND it MUST omit raw prompt text, raw script source, credentials, UCAN proofs, provider payloads, and tool bodies

#### Scenario: denied receipt records safe denial facts [r[steel-execute-turn-authority.receipts.denied]]
- GIVEN execution authority is denied before the host runner
- WHEN the agent reports the failure
- THEN the `steel.host.execute_turn` receipt MUST include a safe denial class and authority receipt hash
- AND it MUST omit raw prompt text, raw script source, credentials, UCAN proofs, provider payloads, and tool bodies

### Requirement: Deterministic verification covers the real seam [r[steel-execute-turn-authority.verification]]

The implementation MUST include deterministic tests and checker evidence for the runtime DTO, real turn-loop path, daemon-visible smoke, redaction, and provider-call denial boundary.

#### Scenario: focused checker writes receipt [r[steel-execute-turn-authority.verification.checker]]
- GIVEN the implementation, tests, docs, and profile artifacts are present
- WHEN the focused checker runs
- THEN it MUST write a deterministic receipt under `target/steel-execute-turn-authority/`
- AND the receipt MUST hash only source artifacts, not secrets or raw prompt/provider/tool payloads

#### Scenario: real smoke proves provider is skipped on denial [r[steel-execute-turn-authority.verification.real-denial]]
- GIVEN a real embedded-controller prompt lacks execution authority but still has planning authority
- WHEN the prompt command runs
- THEN the provider call count MUST stay zero
- AND the daemon-visible execution receipt MUST report the safe denial reason
