# Tasks: Steel Host-Call JSON Payloads

## Phase 1: Implementation

- [x] [serial] I1. Add runtime-owned JSON DTOs/helpers for plan-turn and execute-turn host-call payloads. [covers=r[steel-host-call-json-payloads.plan.valid], r[steel-host-call-json-payloads.execute.valid]]
- [x] [serial] I2. Replace pipe-delimited parsing/construction with JSON serialization/deserialization and fail-closed malformed handling. [covers=r[steel-host-call-json-payloads.plan.legacy-denied], r[steel-host-call-json-payloads.execute.malformed-denied]]
- [x] [serial] I3. Update agent fixtures, embedded smoke expectations, docs, and checker scripts for JSON host-call payloads. [covers=r[steel-host-call-json-payloads.receipts.hashes], r[steel-host-call-json-payloads.verification.checker]]

## Phase 2: Verification

- [x] [serial] V1. Run focused runtime, agent, embedded-controller, and checker validation. [covers=r[steel-host-call-json-payloads.plan.valid], r[steel-host-call-json-payloads.plan.legacy-denied], r[steel-host-call-json-payloads.execute.valid], r[steel-host-call-json-payloads.execute.malformed-denied], r[steel-host-call-json-payloads.receipts.hashes], r[steel-host-call-json-payloads.verification.checker]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Run Cairn gates and repository validation needed before archive. [covers=r[steel-host-call-json-payloads.verification.checker]] [evidence=evidence/cairn-validation.md]
