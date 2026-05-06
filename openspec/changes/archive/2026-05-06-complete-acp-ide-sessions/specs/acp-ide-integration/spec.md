## ADDED Requirements

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
