## ADDED Requirements

### Requirement: Typed process/job notification events [r[process-job-notification-events.schema]]

The system MUST represent process/job completion and readiness notifications as typed events with stable IDs and safe payloads.

#### Scenario: completion event has stable payload [r[process-job-notification-events.schema.completion]]

- GIVEN a job started with `notify_on_complete = true`
- WHEN the job reaches a terminal status
- THEN Clankers MUST create exactly one completion event containing event id, job id, owner scope, backend kind, terminal status, elapsed time where known, safe summary, and log reference or safe excerpt
- THEN the event MUST NOT contain raw secret environment values, headers, or unredacted command/log content

#### Scenario: readiness event identifies pattern without unbounded output [r[process-job-notification-events.schema.readiness]]

- GIVEN a job has bounded watch patterns
- WHEN output matches an allowed readiness pattern
- THEN Clankers MUST create a readiness event containing event id, job id, matched pattern identity, safe excerpt/reference, and timestamp
- THEN the event MUST NOT include unbounded stdout/stderr bytes

### Requirement: Decoupled notification delivery sink [r[process-job-notification-events.delivery]]

The system MUST deliver process/job notification events through an interface that is independent of backend implementations and client transport details.

#### Scenario: backend does not deliver directly [r[process-job-notification-events.delivery.sink-interface]]

- GIVEN a native, pueue, or systemd backend observes a status or log event
- WHEN a notification should be emitted
- THEN backend code MUST return backend facts to the service layer rather than writing TUI, daemon, Matrix, or replay messages directly
- THEN the service layer MUST deliver through a notification sink interface

#### Scenario: multi-client delivery is deduplicated [r[process-job-notification-events.delivery.multi-client-dedup]]

- GIVEN multiple clients are attached to an authorized session or scope
- WHEN a notification event is delivered
- THEN Clankers MUST use stable event ids or dedup keys so each target can avoid duplicate rendering
- THEN delivery to one client MUST NOT suppress delivery or replay for another authorized client

### Requirement: Reattach replay and authorization [r[process-job-notification-events.replay]]

The system MUST persist actionable process/job notification events long enough to replay missed completion/readiness information to authorized reattached clients.

#### Scenario: detached client receives missed completion [r[process-job-notification-events.replay.reattach]]

- GIVEN all clients detach before a notify-on-complete job exits
- WHEN an authorized client later reattaches to the owning session or scope
- THEN Clankers MUST expose the persisted completion event or equivalent replay record
- THEN replay MUST preserve event id and safe log reference/excerpt

#### Scenario: unauthorized reattach cannot replay event [r[process-job-notification-events.replay.unauthorized]]

- GIVEN a client lacks the owner scope or log/observe capability for a job
- WHEN it attaches or requests notification replay
- THEN Clankers MUST omit or deny the event without leaking command, log, or backend details beyond a safe authorization error

### Requirement: Watch-pattern rate limiting and suppression [r[process-job-notification-events.rate-limit]]

The system MUST rate-limit readiness notifications and suppress noisy watch patterns while preserving terminal completion delivery.

#### Scenario: noisy pattern is downgraded [r[process-job-notification-events.rate-limit.watch-patterns]]

- GIVEN a watch pattern repeatedly matches within configured rate-limit windows
- WHEN the match count exceeds the suppression threshold
- THEN Clankers MUST suppress or downgrade further readiness notifications for that pattern
- THEN completion notification delivery MUST remain authoritative when `notify_on_complete` is enabled
