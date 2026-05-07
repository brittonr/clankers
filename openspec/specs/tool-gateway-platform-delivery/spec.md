## Purpose
Define the first-pass tool gateway/platform delivery validation capability, including supported local/session delivery validation, explicit unsupported platform-delivery cases, safe replay metadata, and verification expectations.
## Requirements
### Requirement: Tool Gateway and Platform Delivery Capability [r[tool-gateway-platform-delivery.capability]]
The system MUST provide a first-pass Tool Gateway surface for validating toolset enablement and delivery targets before future platform backends add remote delivery.

#### Scenario: Primary path succeeds [r[tool-gateway-platform-delivery.scenario.primary-path]]
- GIVEN the user invokes `clankers gateway status`, `clankers gateway validate --toolsets <LIST>`, or the Specialty `tool_gateway` tool
- WHEN the delivery target is local/session and the toolsets are recognized
- THEN clankers returns a structured, user-visible success result with normalized toolset and backend metadata

#### Scenario: Unsupported configuration is explicit [r[tool-gateway-platform-delivery.scenario.unsupported-config]]
- GIVEN the user invokes the gateway with remote/platform delivery, Matrix delivery outside an active bridge, webhook, cloud storage, or credential/header delivery
- WHEN clankers cannot safely proceed in the first pass
- THEN clankers MUST return an actionable unsupported error instead of silently falling back or dropping work

### Requirement: Tool Gateway and Platform Delivery Session Observability [r[tool-gateway-platform-delivery.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[tool-gateway-platform-delivery.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN tool-result details include safe metadata such as action, status, backend, normalized toolsets, target kind, support flag, and sanitized error details
- AND tool-result details MUST NOT include webhook URLs, credentials, headers, Matrix room payloads, cloud object URLs, or raw platform payloads

### Requirement: Tool Gateway and Platform Delivery Verification [r[tool-gateway-platform-delivery.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[tool-gateway-platform-delivery.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

### Requirement: Unified Toolset Policy [r[tool-gateway.toolsets]]
The system MUST evaluate enabled toolsets, disabled tools, capability ceilings, and mode restrictions through one shared policy path.

#### Scenario: Standalone and daemon agree [r[tool-gateway.toolsets.scenario.standalone-and-daemon-agree]]
- GIVEN the same settings and command-line toolset choices are used in standalone and daemon sessions
- WHEN tools are built
- THEN both modes expose the same allowed tool names except for explicitly documented transport limitations

#### Scenario: Runtime changes propagate [r[tool-gateway.toolsets.scenario.runtime-changes-propagate]]
- GIVEN a user disables or enables tools during an attached session
- WHEN the command is accepted
- THEN local UI state and daemon state converge without duplicate noisy acknowledgements

### Requirement: Platform-Aware Delivery Receipts [r[tool-gateway.delivery]]
The system MUST route generated files, media, and scheduled-job outputs through platform-aware delivery adapters with safe receipts.

#### Scenario: Media delivery receipt [r[tool-gateway.delivery.scenario.media-delivery-receipt]]
- GIVEN a tool produces a file or media artifact for a platform session
- WHEN delivery runs
- THEN clankers records artifact type, safe path or platform handle, status, and error class without tokens or raw destination secrets

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
