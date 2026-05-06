## Purpose

ACP IDE integration exposes local Clankers sessions to ACP-compatible editors through the foreground stdio adapter while preserving normal session, controller, persistence, and tool-policy boundaries.
## Requirements
### Requirement: ACP IDE Integration Capability [r[acp-ide-integration.capability]]
The system MUST expose clankers sessions through a documented foreground stdio ACP adapter for ACP-compatible editors, and MUST route supported prompt/session requests through existing clankers controller, tool-policy, and session-persistence paths.

#### Scenario: Primary path succeeds [r[acp-ide-integration.scenario.primary-path]]
- GIVEN an ACP-compatible editor launches `clankers acp serve` for a local project
- WHEN the editor sends a supported prompt or new-turn request
- THEN clankers dispatches the request through the normal session/controller path and returns structured ACP-visible progress or output

#### Scenario: Unsupported configuration is explicit [r[acp-ide-integration.scenario.unsupported-config]]
- GIVEN the editor invokes an ACP method or transport that the first-pass adapter does not support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable structured unsupported error instead of silently falling back or dropping work

### Requirement: ACP IDE Integration Session Observability [r[acp-ide-integration.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[acp-ide-integration.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: ACP IDE Integration Verification [r[acp-ide-integration.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[acp-ide-integration.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

### Requirement: ACP Session Binding [r[acp.sessions]]
The system MUST bind ACP clients to real clankers sessions for prompt submission, progress streaming, cancellation, and history replay.

#### Scenario: Create session [r[acp.sessions.scenario.create-session]]
- GIVEN an editor launches clankers acp serve with new-session intent
- WHEN the ACP initialize and prompt flow runs
- THEN clankers creates a session through the ordinary controller path and streams normalized updates

#### Scenario: Attach session [r[acp.sessions.scenario.attach-session]]
- GIVEN an editor supplies an existing session id
- WHEN the ACP client sends a prompt
- THEN clankers routes it to that session or returns a structured missing-session error

### Requirement: ACP Editor Capability Negotiation [r[acp.editor-capabilities]]
The system MUST negotiate editor capabilities for diffs, terminals, workspaces, and tool activity and fail closed for unsupported methods.

#### Scenario: Unsupported terminal explicit [r[acp.editor-capabilities.scenario.unsupported-terminal-explicit]]
- GIVEN the editor requests terminal creation before support is enabled
- WHEN the request is handled
- THEN clankers returns an unsupported-method error without spawning a terminal
