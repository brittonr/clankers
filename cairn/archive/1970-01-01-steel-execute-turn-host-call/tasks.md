# Tasks: Steel Execute Turn Host Call

## Phase 1: Implementation

- [x] [serial] I1. Add runtime-owned execute-turn host-call source/payload/receipt types and approval/denial/malformed tests. [covers=r[steel-execute-turn-host-call.runtime.allowed], r[steel-execute-turn-host-call.runtime.denied], r[steel-execute-turn-host-call.runtime.malformed]]
- [x] [serial] I2. Wire `authorize_steel_turn_execution` to require both the Steel host-call receipt and dynamic-runtime authorization before returning authorized execution. [covers=r[steel-execute-turn-host-call.runtime.allowed], r[steel-execute-turn-host-call.runtime.denied], r[steel-execute-turn-host-call.runtime.malformed]]
- [x] [serial] I3. Extend agent daemon-visible execution receipts and embedded smoke assertions with safe host-call fields. [covers=r[steel-execute-turn-host-call.receipts.allowed], r[steel-execute-turn-host-call.receipts.denied], r[steel-execute-turn-host-call.verification.real-denial]]
- [x] [serial] I4. Update docs and focused checker coverage for the execute-turn host-call contract. [covers=r[steel-execute-turn-host-call.verification.checker]]

## Phase 2: Verification

- [x] [serial] V1. Run focused runtime, agent, embedded-controller, and checker validation. [covers=r[steel-execute-turn-host-call.runtime.allowed], r[steel-execute-turn-host-call.runtime.denied], r[steel-execute-turn-host-call.runtime.malformed], r[steel-execute-turn-host-call.receipts.allowed], r[steel-execute-turn-host-call.receipts.denied], r[steel-execute-turn-host-call.verification.checker], r[steel-execute-turn-host-call.verification.real-denial]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Run Cairn gates and repository validation needed before archive. [covers=r[steel-execute-turn-host-call.verification.checker]] [evidence=evidence/cairn-validation.md]
