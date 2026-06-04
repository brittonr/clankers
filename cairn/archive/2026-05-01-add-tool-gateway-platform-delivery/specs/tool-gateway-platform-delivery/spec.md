## ADDED Requirements

### Requirement: Tool Gateway and Platform Delivery Capability [r[tool-gateway-platform-delivery.capability]]
The system MUST provide Unify toolset enablement, platform-scoped media/file delivery, and scheduled-task delivery targets.

#### Scenario: Primary path succeeds [r[tool-gateway-platform-delivery.scenario.primary-path]]
- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[tool-gateway-platform-delivery.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: Tool Gateway and Platform Delivery Session Observability [r[tool-gateway-platform-delivery.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[tool-gateway-platform-delivery.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: Tool Gateway and Platform Delivery Verification [r[tool-gateway-platform-delivery.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[tool-gateway-platform-delivery.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure
