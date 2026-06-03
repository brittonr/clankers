## ADDED Requirements

### Requirement: Actor loop receives assembled services [r[daemon-actor-loop-service-drain.loop-inputs]]

Daemon actor loops MUST poll commands, signals, confirmations, plugin events, and controller events using assembled service inputs rather than constructing unrelated runtime policy inline.

#### Scenario: loop inputs are explicit [r[daemon-actor-loop-service-drain.loop-inputs.explicit]]
- GIVEN a daemon session actor starts
- WHEN the actor loop begins
- THEN service inputs for selected responsibilities MUST already be assembled
- AND actor-loop code MUST not own that selected policy inline

### Requirement: Extracted daemon services own one responsibility [r[daemon-actor-loop-service-drain.service-owner]]

Plugin UI draining, schedule handling, tool-list sync, prompt RPC collection, or similar daemon responsibilities MUST live in focused services when extracted.

#### Scenario: selected service is socketless-testable [r[daemon-actor-loop-service-drain.service-owner.socketless]]
- GIVEN the selected daemon responsibility is tested
- WHEN focused fixtures run
- THEN they MUST not require binding a Unix socket or running a full actor registry

### Requirement: Socketless fixtures cover construction decisions [r[daemon-actor-loop-service-drain.socketless-fixtures]]

Daemon service extraction MUST include deterministic fixtures that prove construction and tick behavior.

#### Scenario: fixture covers actor call path [r[daemon-actor-loop-service-drain.socketless-fixtures.call-path]]
- GIVEN the actor loop invokes the service
- WHEN tests run
- THEN they MUST cover both the service behavior and the actor call path into the service

### Requirement: Daemon extraction validation passes [r[daemon-actor-loop-service-drain.verification]]

Focused daemon tests and architecture rails MUST pass after service extraction.

#### Scenario: rails reject policy regression [r[daemon-actor-loop-service-drain.verification.rails]]
- GIVEN daemon actor code is inspected
- WHEN the selected responsibility returns inline
- THEN the rail MUST fail and name the service owner
