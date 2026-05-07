## ADDED Requirements

### Requirement: Adapter-Backed Platform Delivery [r[tool-gateway.delivery.adapters]]
The system MUST execute approved artifact delivery through a shared Tool Gateway delivery adapter boundary rather than emitting receipt-only success for platform targets.

#### Scenario: Approved adapter executes delivery [r[tool-gateway.delivery.adapters.scenario.approved-executes]]
- GIVEN an artifact handle, artifact kind, active session context, and delivery target pass Tool Gateway policy validation
- WHEN delivery is requested
- THEN clankers MUST create a delivery attempt, call the selected adapter, and record a safe receipt with attempt id, status, target kind, artifact kind, safe artifact label, and optional platform handle

#### Scenario: Unsupported target fails before adapter execution [r[tool-gateway.delivery.adapters.scenario.unsupported-before-exec]]
- GIVEN a delivery target has no configured adapter, lacks required session context, or includes raw destination/credential material
- WHEN delivery is requested
- THEN clankers MUST reject the request before adapter execution and record only replay-safe error metadata

### Requirement: Delivery Outbox and Retry [r[tool-gateway.delivery.outbox]]
The system MUST persist bounded delivery attempts in a local outbox so failed platform delivery can be inspected and retried without resubmitting raw destinations or payloads.

#### Scenario: Retry uses attempt id [r[tool-gateway.delivery.outbox.scenario.retry-attempt-id]]
- GIVEN a prior delivery attempt failed with a retryable adapter error
- WHEN an operator requests retry
- THEN clankers MUST reference the stored attempt id, revalidate current policy, and avoid requiring or exposing raw destination secrets

#### Scenario: Non-retryable failures stay closed [r[tool-gateway.delivery.outbox.scenario.non-retryable]]
- GIVEN an attempt failed because policy rejected the target, artifact, credentials, or session context
- WHEN retry is requested
- THEN clankers MUST refuse retry until the underlying policy blocker changes and MUST preserve the original safe failure class

### Requirement: Matrix-Bound Delivery [r[tool-gateway.delivery.matrix]]
The system MUST support Matrix artifact delivery only when the target is bound to an explicit active Matrix bridge/session context.

#### Scenario: Matrix session context delivers [r[tool-gateway.delivery.matrix.scenario.active-context]]
- GIVEN clankers is running inside an authenticated Matrix bridge session with a delivery-capable context
- WHEN a file, media, or scheduled-output artifact is delivered to the Matrix target kind
- THEN clankers MUST send through the Matrix adapter and record a safe platform handle without persisting room secrets, access tokens, or raw message payloads

#### Scenario: Raw Matrix destinations are rejected [r[tool-gateway.delivery.matrix.scenario.reject-raw-destination]]
- GIVEN a delivery request supplies a raw room id, homeserver URL, access token, or message payload outside the active bridge context
- WHEN Tool Gateway policy validates the request
- THEN clankers MUST reject it before delivery and include only target kind plus error class in the receipt
