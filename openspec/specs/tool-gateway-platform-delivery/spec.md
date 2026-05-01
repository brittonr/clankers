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
