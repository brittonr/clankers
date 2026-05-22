# Steel Turn Planning Runtime Smoke Specification

## Purpose

Defines deterministic smoke evidence for the already-landed Steel Scheme `steel.host.plan_turn` configuration activation path at a real Clankers session boundary.

## Requirements

### Requirement: Config activation reaches real session prompt handling [r[steel-turn-planning-runtime-smoke.session-activation]]

Clankers MUST prove that reviewed Steel turn-planning settings can activate the planner through real session prompt handling rather than only through direct helper tests.

#### Scenario: prompt command activates reviewed planner
- GIVEN a session controller is built with Steel turn planning enabled from reviewed profile and script paths
- WHEN a prompt command is handled
- THEN the constructed agent turn MUST use the Steel turn-planning configuration
- AND the prompt MUST still execute through the Rust-owned provider path

### Requirement: Receipt is client-visible and redacted [r[steel-turn-planning-runtime-smoke.visible-receipt]]

Clankers MUST emit client-observable receipt evidence for Steel turn-planning decisions without leaking prompt bodies, secrets, or script bodies.

#### Scenario: daemon-visible event carries redacted receipt
- GIVEN the reviewed Steel planner authorizes the `steel.host.plan_turn` seam
- WHEN the prompt finishes
- THEN daemon/session-visible events MUST include a Steel plan-turn receipt status
- AND the receipt text MUST omit the raw user prompt
- AND the receipt text MUST omit raw script/profile bodies

### Requirement: Invalid runtime activation fails closed [r[steel-turn-planning-runtime-smoke.fail-closed]]

Clankers MUST not silently fall back to Rust-only planning when reviewed Steel activation config is invalid at runtime.

#### Scenario: hash mismatch blocks prompt completion
- GIVEN settings declare an expected Steel script or profile hash that does not match the reviewed artifact
- WHEN a prompt command is handled
- THEN the prompt MUST report an error
- AND no successful Steel planning receipt may be emitted

#### Scenario: missing authority blocks prompt completion
- GIVEN the reviewed profile requires a session capability or UCAN ability that settings do not grant
- WHEN a prompt command is handled
- THEN the prompt MUST report an error
- AND no provider call may be required to mask the activation failure

### Requirement: Smoke rail records deterministic local evidence [r[steel-turn-planning-runtime-smoke.receipt-rail]]

Clankers MUST provide a deterministic local checker receipt for the smoke rail.

#### Scenario: checker writes target receipt
- GIVEN the smoke rail source, docs, and Cairn tasks are present
- WHEN the checker runs
- THEN it MUST write a receipt under `target/steel-turn-planning-runtime-smoke/`
- AND the receipt MUST hash reviewed artifacts without embedding raw prompts, secrets, scripts, or profile bodies

### Requirement: Smoke rail is verified and archived [r[steel-turn-planning-runtime-smoke.verification]]

Clankers MUST verify and archive this smoke rail before landing it.

#### Scenario: focused gates pass before archive
- GIVEN the smoke tests and checker are implemented
- WHEN the change is finalized
- THEN focused Rust tests, checker, Cairn validation/gates, and whitespace checks MUST pass

### Requirement: Change is archived onto clean main [r[steel-turn-planning-runtime-smoke.archive]]

Clankers MUST sync/archive the Cairn change and land it on clean pushed `main`.

#### Scenario: archived accepted spec remains durable
- GIVEN implementation verification passes
- WHEN the Cairn is synced and archived
- THEN the accepted spec MUST retain the smoke requirements
- AND `main` MUST be pushed with a clean working tree
