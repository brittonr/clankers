## ADDED Requirements

### Requirement: Browser Automation Capability [r[browser-automation.capability]]
The system MUST provide stateful browser sessions through a documented `browser` tool using a local Chrome/Chromium CDP-compatible backend first, with an API shape that can support remote providers later.

#### Scenario: Primary path succeeds [r[browser-automation.scenario.primary-path]]
- GIVEN `browserAutomation.enabled` is true and the configured backend is available
- WHEN the user or agent invokes the documented `browser` tool action such as `navigate` or `snapshot`
- THEN clankers performs the operation and returns a structured, user-visible result including the session id, action, backend, status, and safe page metadata when available

#### Scenario: Stateful session can be reused [r[browser-automation.scenario.stateful-session]]
- GIVEN a browser action opened or selected a session
- WHEN a later browser action passes the same `sessionId`
- THEN clankers MUST target the same logical browser session unless it was explicitly closed or failed

#### Scenario: Unsupported configuration is explicit [r[browser-automation.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back to stateless web fetch or dropping work

### Requirement: Browser Automation Configuration and Policy [r[browser-automation.config-policy]]
The system MUST expose typed `browserAutomation` settings that control backend selection, local CDP connection or launch details, profile location, action policy, origin restrictions, and timeouts.

#### Scenario: Disabled by default [r[browser-automation.scenario.disabled-default]]
- GIVEN no browser automation settings are configured
- WHEN clankers builds the default tool list
- THEN the `browser` tool is not published by default

#### Scenario: Enabled CDP configuration validates [r[browser-automation.scenario.cdp-config-validates]]
- GIVEN `browserAutomation.enabled` is true and `backend` is `cdp`
- WHEN either `cdpUrl` or `browserBinary` is configured with a positive timeout
- THEN settings validation succeeds and the `browser` tool can be published as a Specialty tool

#### Scenario: Policy-gated actions fail safely [r[browser-automation.scenario.policy-gated-actions]]
- GIVEN `allowEvaluate` or `allowScreenshots` is false
- WHEN the user or agent invokes the corresponding `evaluate` or `screenshot` action
- THEN clankers MUST reject the action with a policy error before contacting the browser backend

#### Scenario: Origin policy is enforced [r[browser-automation.scenario.origin-policy]]
- GIVEN `allowedOrigins` is configured
- WHEN a navigation targets a URL whose origin does not match the allowlist
- THEN clankers MUST reject the navigation with an actionable policy error

### Requirement: Browser Automation Session Observability [r[browser-automation.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[browser-automation.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the tool result details include source `browser_automation`, backend identity, action, session id, status, elapsed time or timing when available, safe URL/origin when available, and redacted error details when applicable

### Requirement: Browser Automation Verification [r[browser-automation.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[browser-automation.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful browser operation and one policy/configuration failure without requiring a real Chromium installation
