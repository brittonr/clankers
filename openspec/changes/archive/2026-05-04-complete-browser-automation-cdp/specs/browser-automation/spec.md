## ADDED Requirements

### Requirement: Local CDP Browser Runtime [r[browser-cdp.runtime]]
The system MUST provide a local CDP-compatible browser runtime for enabled browser automation configurations.

#### Scenario: Launch or connect [r[browser-cdp.runtime.scenario.launch-or-connect]]
- GIVEN browserAutomation is enabled with either a CDP URL or browser binary
- WHEN the runtime initializes
- THEN clankers connects to or launches a browser session and returns a stable session identifier

#### Scenario: Reuse session [r[browser-cdp.runtime.scenario.reuse-session]]
- GIVEN a prior browser action returned a session identifier
- WHEN a later action uses that identifier
- THEN clankers targets the same browser page unless it was closed or failed

### Requirement: Stateful Browser Actions [r[browser-cdp.actions]]
The system MUST support policy-checked navigate, current_url, snapshot, click, type, screenshot, evaluate, and close actions through one tool schema.

#### Scenario: Navigation and dom action [r[browser-cdp.actions.scenario.navigation-and-dom-action]]
- GIVEN a page is open and origin policy permits it
- WHEN the agent invokes navigate, click, or type with required fields
- THEN clankers executes the action or returns a selector/action error with safe page metadata

#### Scenario: Evaluate remains gated [r[browser-cdp.actions.scenario.evaluate-remains-gated]]
- GIVEN allowEvaluate is false
- WHEN the agent invokes evaluate
- THEN clankers rejects the action before sending script text to the browser

### Requirement: Browser CDP Verification [r[browser-cdp.verification]]
The implementation MUST include deterministic tests and optional live smoke coverage for the concrete CDP backend.

#### Scenario: Fake runtime covers contracts [r[browser-cdp.verification.scenario.fake-runtime-covers-contracts]]
- GIVEN the regression suite runs without Chromium
- WHEN browser automation tests execute
- THEN the suite covers publication, happy path, policy rejection, and backend failure receipts
