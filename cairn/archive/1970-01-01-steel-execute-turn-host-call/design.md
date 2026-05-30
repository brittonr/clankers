# Design: Steel Execute Turn Host Call

## Overview

The runtime already models Steel host calls through `SteelRuntimeRequest` and `SteelHostFunctionRegistration`. This change adds a second execution-time runtime evaluation with source `(host "steel.host.execute_turn")`. The registered host function returns a compact typed payload that references the plan receipt and host runner. Rust validates that payload and records a redacted host-call receipt before applying existing dynamic-runtime authorization.

## Runtime Contract

Add runtime-owned data:

- `STEEL_TURN_EXECUTION_HOST_CALL_SCHEMA`
- `DEFAULT_TURN_EXECUTION_SOURCE`
- `SteelTurnExecutionHostCallReceipt`

`authorize_steel_turn_execution(...)` will:

1. Hash the metadata-only `SteelTurnExecutionInput`.
2. Evaluate a `SteelRuntimeRequest` using `DEFAULT_TURN_EXECUTION_SOURCE`.
3. Register `steel.host.execute_turn` only when the reviewed profile allows that host action.
4. Require an execution session capability for the host call.
5. Validate the host-call output payload against the expected schema, seam, plan receipt hash, and host-runner label.
6. Run existing dynamic-runtime authorization.
7. Return `SteelTurnExecutionStatus::Authorized` only when both the Steel host call and dynamic-runtime authorization allow execution.

## Agent Receipt Surface

The daemon-visible `steel.host.execute_turn` system message keeps the existing authority fields and adds safe host-call fields:

- `host_call_status`
- `host_call_reason`
- `host_call_payload`
- `host_call_receipt_hash`

The receipt remains metadata-only: raw prompts, script source, provider payloads, tool bodies, credentials, and UCAN proofs stay absent.

## Failure Modes

- Missing execution session capability denies at the Steel host-call layer and dynamic-runtime layer; provider call count remains zero.
- Disabled `steel.host.execute_turn` denies at both layers.
- Missing/unsupported profile host action denies the host call before effects.
- Malformed host-call output denies execution even if dynamic-runtime authorization would otherwise allow it.

## Verification

Use focused runtime tests for host-call allowed/denied/malformed behavior, existing real turn-loop tests for Steel-selected execution, embedded-controller smoke for daemon-visible denial before provider, and a static checker that hashes the touched source/doc artifacts.
