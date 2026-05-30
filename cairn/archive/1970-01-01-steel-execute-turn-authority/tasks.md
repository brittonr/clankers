# Tasks: Steel Execute Turn Authority

## Phase 1: Implementation

- [x] [serial] I1. Add `steel.host.execute_turn` to the reviewed default profile/policy surface with separate session capability and UCAN ability. [covers=r[steel-execute-turn-authority.profile.separate-grants]]
- [x] [serial] I2. Add runtime-owned `SteelTurnExecutionInput` / `SteelTurnExecutionReceipt` and dynamic-runtime authorization for the execution seam. [covers=r[steel-execute-turn-authority.pre-run.allowed], r[steel-execute-turn-authority.receipts.allowed]]
- [x] [serial] I3. Wire the Steel-selected agent execution adapter to authorize before the host runner and emit allowed/denied daemon-visible receipts. [covers=r[steel-execute-turn-authority.pre-run.allowed], r[steel-execute-turn-authority.pre-run.denied], r[steel-execute-turn-authority.receipts.allowed], r[steel-execute-turn-authority.receipts.denied]]
- [x] [serial] I4. Extend runtime, real turn-loop, and embedded-controller smoke tests plus docs/checker markers for allowed and denied execution authority. [covers=r[steel-execute-turn-authority.verification.checker], r[steel-execute-turn-authority.verification.real-denial]]

## Phase 2: Verification

- [x] [serial] V1. Run focused runtime, turn-loop, embedded-controller, and static checker validation. [covers=r[steel-execute-turn-authority.profile.separate-grants], r[steel-execute-turn-authority.pre-run.allowed], r[steel-execute-turn-authority.pre-run.denied], r[steel-execute-turn-authority.receipts.allowed], r[steel-execute-turn-authority.receipts.denied], r[steel-execute-turn-authority.verification.checker], r[steel-execute-turn-authority.verification.real-denial]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Run Cairn proposal/design/tasks gates plus repository validation needed before archive. [covers=r[steel-execute-turn-authority.verification.checker]] [evidence=evidence/cairn-validation.md]
