# Steel Host-Call JSON Payloads Delta

## Purpose

Moves Steel turn-planning and execute-turn host-call payloads from pipe-delimited fixture strings to runtime-owned typed JSON DTOs.

## Requirements

### Requirement: Turn planning host-call payload is JSON [r[steel-host-call-json-payloads.plan]]

The `steel.host.plan_turn` host-call output MUST be parsed as a typed JSON DTO with an explicit schema before Rust builds an `OrchestrationPlan`.

#### Scenario: valid planning JSON selects a candidate [r[steel-host-call-json-payloads.plan.valid]]
- GIVEN Steel returns JSON with the expected schema, decision id, action name, target resource, and decision class
- WHEN Rust parses the `steel.host.plan_turn` output
- THEN it MUST select the matching candidate and build the authorized Steel plan

#### Scenario: legacy delimited planning output fails closed [r[steel-host-call-json-payloads.plan.legacy-denied]]
- GIVEN Steel returns the former pipe-delimited plan string
- WHEN Rust parses the `steel.host.plan_turn` output
- THEN it MUST reject the payload as malformed
- AND fallback/block behavior MUST follow the existing orchestration policy

### Requirement: Execute-turn host-call payload is JSON [r[steel-host-call-json-payloads.execute]]

The `steel.host.execute_turn` host-call output MUST be parsed as a typed JSON DTO with an explicit schema before Rust authorizes execution.

#### Scenario: valid execute-turn JSON allows host-call receipt [r[steel-host-call-json-payloads.execute.valid]]
- GIVEN Steel returns JSON with the expected schema, seam, plan receipt hash, host-runner label, and execution input hash
- WHEN Rust validates the execute-turn host-call payload
- THEN the host-call receipt MUST mark the payload valid
- AND execution may proceed only if dynamic-runtime authorization also allows it

#### Scenario: malformed execute-turn JSON denies execution [r[steel-host-call-json-payloads.execute.malformed-denied]]
- GIVEN Steel returns invalid JSON, wrong schema, wrong seam, wrong plan hash, wrong host-runner label, or wrong input hash
- WHEN Rust validates the execute-turn host-call payload
- THEN execution MUST be denied before provider/tool host effects run

### Requirement: JSON payload receipts remain redacted and deterministic [r[steel-host-call-json-payloads.receipts]]

Receipts and checker artifacts MUST expose only safe schema/hash/status facts for JSON host-call payloads.

#### Scenario: receipts hash typed JSON payloads [r[steel-host-call-json-payloads.receipts.hashes]]
- GIVEN host-call payload JSON is valid or invalid
- WHEN receipts are emitted
- THEN receipts MUST include safe payload validity and payload hash fields
- AND they MUST omit raw prompts, provider payloads, tool bodies, credentials, and UCAN proofs

### Requirement: Deterministic validation covers JSON payload seams [r[steel-host-call-json-payloads.verification]]

The implementation MUST include focused tests and checker evidence for planning JSON, execute-turn JSON, malformed legacy payload rejection, and real embedded-controller receipt behavior.

#### Scenario: focused checker writes receipt [r[steel-host-call-json-payloads.verification.checker]]
- GIVEN implementation, docs, tests, and Cairn artifacts are present
- WHEN the focused checker runs
- THEN it MUST write a deterministic receipt under `target/steel-host-call-json-payloads/`
- AND the receipt MUST hash only source artifacts, not raw prompt/provider/tool payloads or secrets
