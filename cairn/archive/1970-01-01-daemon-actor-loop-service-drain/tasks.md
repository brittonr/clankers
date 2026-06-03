## Phase 1: Implementation

- [x] [serial] I1: Inventory daemon actor-loop responsibilities and choose the background projection/tick drain as the service extraction target. r[daemon-actor-loop-service-drain.loop-inputs] [covers=daemon-actor-loop-service-drain.loop-inputs]
- [x] [serial] I2: Move selected tool-list sync, controller-event drain, and plugin-runtime UI drain policy into `DaemonSessionTickService`. r[daemon-actor-loop-service-drain.service-owner] [covers=daemon-actor-loop-service-drain.service-owner]
- [x] [serial] I3: Add socketless tick-service fixtures and keep actor-loop call path thin through the assembled service. r[daemon-actor-loop-service-drain.socketless-fixtures] [covers=daemon-actor-loop-service-drain.socketless-fixtures]
- [x] [serial] I4: Update architecture rails to name `DaemonSessionTickService` and reject selected drain policy returning to `agent_process.rs`. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification]

## Phase 2: Verification

- [x] [serial] V1: Run focused daemon service/actor tests for the moved responsibility. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification] [evidence=evidence/tick-service-drain.md]
- [x] [serial] V2: Run relevant daemon/attach parity tests, architecture rails, Cairn gates/validate, and `git diff --check`. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification] [evidence=evidence/tick-service-drain.md]
