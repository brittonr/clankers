# Design: Steel Host-Call JSON Payloads

## Overview

The runtime will own two JSON DTOs:

- `SteelTurnPlanHostCallPayload`
- `SteelTurnExecutionHostCallPayload`

Each DTO includes an explicit schema string and only metadata-safe fields. Rust serializes these structs with `serde_json` and parses them back at the host-call boundary. The DTO field order is fixed by the struct definition for deterministic source-controlled fixtures and hash material.

## Planning Payload

`steel.host.plan_turn` returns JSON with:

- `schema`
- `decision_id`
- `action_name`
- `target_resource`
- `decision_class`

`parse_steel_plan_payload(...)` deserializes this DTO, validates the schema, finds the matching candidate, and builds the typed `OrchestrationPlan`.

## Execution Payload

`steel.host.execute_turn` returns JSON with:

- `schema`
- `seam`
- `plan_receipt_hash`
- `host_runner`
- `input_hash`

The execution host-call receipt validates exact schema/seam/plan hash/host-runner/input hash before allowing dynamic-runtime authorization to produce an authorized execution status.

## Compatibility

Legacy pipe-delimited payloads are intentionally not accepted at the new seam. Malformed payloads fail closed with the existing safe malformed-payload denial path.

## Verification

Focused runtime tests prove JSON payload acceptance and malformed legacy-string rejection. Agent and embedded smoke tests continue to prove the real turn-loop and provider-skip denial paths. Static checker scripts assert JSON DTOs, helper use, and receipt redaction markers.
