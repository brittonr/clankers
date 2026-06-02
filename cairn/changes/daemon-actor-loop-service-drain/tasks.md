## Phase 1: Implementation

- [ ] [serial] I1: Inventory daemon actor-loop responsibilities and choose one service extraction target. r[daemon-actor-loop-service-drain.loop-inputs] [covers=daemon-actor-loop-service-drain.loop-inputs]
- [ ] [serial] I2: Move the selected responsibility into an assembled daemon service or tick adapter. r[daemon-actor-loop-service-drain.service-owner] [covers=daemon-actor-loop-service-drain.service-owner]
- [ ] [serial] I3: Add socketless fixtures for the new service and keep actor-loop call path thin. r[daemon-actor-loop-service-drain.socketless-fixtures] [covers=daemon-actor-loop-service-drain.socketless-fixtures]
- [ ] [serial] I4: Update architecture rails to name the service owner and reject policy returning to `agent_process.rs`. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification]

## Phase 2: Verification

- [ ] [serial] V1: Run focused daemon service/actor tests for the moved responsibility. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification]
- [ ] [serial] V2: Run relevant daemon/attach parity tests, architecture rails, Cairn gates/validate, and `git diff --check`. r[daemon-actor-loop-service-drain.verification] [covers=daemon-actor-loop-service-drain.verification]
