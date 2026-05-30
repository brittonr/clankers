# Steel Execute Turn Host Call Delta

## Purpose

Requires Steel-selected execution to pass through an explicit `steel.host.execute_turn` Steel runtime host-call contract before Rust runs provider/tool host effects.

## Requirements

### Requirement: Execution host call is runtime-owned [r[steel-execute-turn-host-call.runtime]]

Rust MUST model `steel.host.execute_turn` as a runtime-owned Steel host-call request and receipt before the Rust host runner is invoked.

#### Scenario: allowed host call precedes dynamic authorization [r[steel-execute-turn-host-call.runtime.allowed]]
- GIVEN the reviewed default profile allows `steel.host.execute_turn`
- AND the session has the execution host-call capability
- WHEN `authorize_steel_turn_execution` evaluates the execution seam
- THEN it MUST evaluate `(host "steel.host.execute_turn")` through the constrained Steel runtime wrapper
- AND it MUST record an approved host-call receipt before returning authorized execution

#### Scenario: denied host call blocks execution [r[steel-execute-turn-host-call.runtime.denied]]
- GIVEN the session lacks execution host-call capability, disables the seam, or the profile omits the execution host action
- WHEN the Steel-selected execution adapter evaluates the execution seam
- THEN the runtime host-call receipt MUST be denied
- AND the Rust host runner MUST NOT be called

#### Scenario: malformed host-call payload blocks execution [r[steel-execute-turn-host-call.runtime.malformed]]
- GIVEN the Steel host call returns a payload that does not match the expected execution schema, seam, plan receipt hash, or host-runner label
- WHEN Rust validates the host-call payload
- THEN execution MUST be denied before provider/tool host effects run

### Requirement: Execution receipts expose host-call facts safely [r[steel-execute-turn-host-call.receipts]]

Daemon-visible execution receipts MUST include safe host-call status facts in addition to dynamic-runtime authority facts.

#### Scenario: allowed receipt includes host-call hash [r[steel-execute-turn-host-call.receipts.allowed]]
- GIVEN execution host-call and dynamic-runtime authorization both allow the turn
- WHEN the Rust host runner completes
- THEN the `steel.host.execute_turn` receipt MUST include host-call status, reason, payload validation class, host-call receipt hash, authority receipt hash, safe counts, and final execution receipt hash
- AND it MUST omit raw prompt text, script source, provider payloads, tool bodies, credentials, and UCAN proofs

#### Scenario: denied receipt includes safe host-call denial [r[steel-execute-turn-host-call.receipts.denied]]
- GIVEN the execution host-call is denied before the Rust host runner
- WHEN the agent reports the failure
- THEN the `steel.host.execute_turn` receipt MUST include the safe host-call denial class and receipt hash
- AND it MUST omit raw prompt text, script source, provider payloads, tool bodies, credentials, and UCAN proofs

### Requirement: Deterministic verification covers the real host-call seam [r[steel-execute-turn-host-call.verification]]

The implementation MUST include deterministic tests and checker evidence for runtime host-call approval, denial, malformed payload handling, daemon-visible receipts, and provider-call skipping on denial.

#### Scenario: focused checker writes receipt [r[steel-execute-turn-host-call.verification.checker]]
- GIVEN implementation, docs, tests, and Cairn artifacts are present
- WHEN the focused checker runs
- THEN it MUST write a deterministic receipt under `target/steel-execute-turn-host-call/`
- AND the receipt MUST hash only source artifacts, not raw prompt/provider/tool payloads or secrets

#### Scenario: embedded smoke proves denied host call skips provider [r[steel-execute-turn-host-call.verification.real-denial]]
- GIVEN a real embedded-controller prompt lacks execution host-call authority while planning succeeds
- WHEN the prompt command runs
- THEN provider call count MUST stay zero
- AND the daemon-visible execution receipt MUST report the safe host-call denial reason
