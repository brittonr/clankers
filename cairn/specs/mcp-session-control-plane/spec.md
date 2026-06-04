# MCP Session Control Plane Specification

## Purpose

This specification defines the local MCP bridge that lets external MCP clients observe and steer clankers daemon sessions through the same session-command and event substrate used by TUI and attach clients.

## Requirements
### Requirement: Local MCP Session Control Bridge [r[mcp-session-control-plane.bridge]]
The system MUST expose a local MCP bridge that allows external MCP clients to observe and steer clankers daemon sessions through the same session protocol used by TUI and attach clients.

#### Scenario: Local MCP bridge starts [r[mcp-session-control-plane.bridge.scenario.starts]]
- GIVEN the user launches the documented local MCP bridge command
- WHEN the bridge initializes over stdio
- THEN it MUST advertise only the supported session-control tools/resources and MUST report unsupported transports or methods as structured errors

#### Scenario: Bridge attaches to an existing session [r[mcp-session-control-plane.bridge.scenario.attach-existing]]
- GIVEN a clankers daemon session exists
- WHEN an MCP client selects or supplies that session id
- THEN the bridge MUST attach using the normal daemon/session client substrate and stream normal session events back to the MCP client

#### Scenario: Missing session is explicit [r[mcp-session-control-plane.bridge.scenario.missing-session]]
- GIVEN an MCP client targets a missing or expired session
- WHEN the bridge attempts to attach or act
- THEN it MUST return an actionable error and MUST NOT create hidden replacement sessions unless the tool explicitly documents create-or-resume behavior

### Requirement: MCP Command Parity [r[mcp-session-control-plane.command-parity]]
Every MCP mutation MUST map to the same `SessionCommand` and daemon policy path that an equivalent human TUI, attach, slash-command, or client action uses.

#### Scenario: Prompt submission follows normal path [r[mcp-session-control-plane.command-parity.scenario.prompt]]
- GIVEN an MCP client calls the prompt-submission tool with text and optional images
- WHEN the bridge accepts the request
- THEN it MUST emit `SessionCommand::Prompt` through the session client path and MUST NOT call the agent/controller directly

#### Scenario: Thinking level follows normal path [r[mcp-session-control-plane.command-parity.scenario.thinking]]
- GIVEN an MCP client requests a thinking-level change
- WHEN the bridge accepts the request
- THEN it MUST emit the same explicit or cycle thinking command used by attach/TUI and observe the same daemon acknowledgment behavior

#### Scenario: Confirmation response follows normal path [r[mcp-session-control-plane.command-parity.scenario.confirmation]]
- GIVEN a pending confirmation request exists in the session event stream
- WHEN an MCP client approves or denies it
- THEN the bridge MUST emit the normal confirmation response command and MUST preserve the same approval/denial semantics as the TUI

### Requirement: MCP Session Tool Surface [r[mcp-session-control-plane.tool-surface]]
The first-pass MCP bridge MUST expose a minimal allowlisted session-control surface and MUST return explicit unsupported errors for operations outside that surface.

#### Scenario: Initial tools are allowlisted [r[mcp-session-control-plane.tool-surface.scenario.initial-tools]]
- GIVEN the MCP bridge is initialized
- WHEN the client lists tools
- THEN the bridge MUST expose prompt submission, abort/interrupt, thinking-level update, disabled-tool or capability update, confirmation response, compaction, session status, and recent event/history read tools if their backing session commands are available

#### Scenario: Unsupported private operation is denied [r[mcp-session-control-plane.tool-surface.scenario.unsupported-private]]
- GIVEN an MCP client asks to mutate TUI-local widget state, inject raw terminal input, call a private controller method, or bypass daemon/session policy
- WHEN the bridge evaluates the request
- THEN it MUST reject the request with a structured unsupported or forbidden error

### Requirement: MCP No-Bypass Boundary [r[mcp-session-control-plane.no-bypass]]
The implementation MUST prevent MCP from bypassing confirmations, disabled-tool policy, capability ceilings, session persistence, event replay, or the normal daemon/session authority boundary.

#### Scenario: Dangerous action still needs confirmation [r[mcp-session-control-plane.no-bypass.scenario.confirmation-required]]
- GIVEN a session action would require confirmation for a human user
- WHEN the same action is requested through MCP
- THEN clankers MUST present or require the same confirmation flow before the action proceeds

#### Scenario: Capability ceiling cannot be exceeded [r[mcp-session-control-plane.no-bypass.scenario.capability-ceiling]]
- GIVEN a session has a configured capability or disabled-tool ceiling
- WHEN an MCP client requests broader tools or capabilities
- THEN clankers MUST reject the request through the normal session policy path

#### Scenario: No direct TUI mutation [r[mcp-session-control-plane.no-bypass.scenario.no-tui-mutation]]
- GIVEN a TUI is attached to the same session
- WHEN MCP mutates session state
- THEN the TUI MUST observe the change through normal daemon events rather than through direct mutation of TUI application state

### Requirement: MCP Receipts and Observability [r[mcp-session-control-plane.receipts]]
MCP mutations MUST return structured receipts and expose safe session resources that are sufficient for audit, replay, and orchestration without leaking secrets.

#### Scenario: Mutation receipt is event-backed [r[mcp-session-control-plane.receipts.scenario.mutation-receipt]]
- GIVEN an MCP mutation is accepted
- WHEN the bridge returns a result
- THEN the result MUST include source, session id, action, command identity, status, timestamp, and event/state evidence when available

#### Scenario: Metadata is safe [r[mcp-session-control-plane.receipts.scenario.safe-metadata]]
- GIVEN a receipt or session resource is persisted or returned
- WHEN it includes diagnostic metadata
- THEN it MUST avoid raw credentials, environment values, provider payloads, unrequested prompt bodies, and unredacted secret-like error text

### Requirement: MCP User-Substrate Parity Tests [r[mcp-session-control-plane.parity-tests]]
The implementation MUST include automated regression tests proving MCP and user-facing paths converge on the same command/event substrate.

#### Scenario: Supported actions have parity coverage [r[mcp-session-control-plane.parity-tests.scenario.supported-actions]]
- GIVEN a supported MCP mutation has an equivalent TUI, attach, slash-command, or client action
- WHEN tests exercise both paths
- THEN they MUST assert equivalent `SessionCommand` emission, daemon event observation, persistence metadata, or confirmation behavior as applicable

### Requirement: MCP Session-Control Documentation [r[mcp-session-control-plane.documentation]]
The implementation MUST document local MCP bridge setup, supported operations, safety boundaries, receipts, and unsupported behavior.

#### Scenario: Docs describe the trust boundary [r[mcp-session-control-plane.documentation.scenario.boundary]]
- GIVEN a user reads the MCP session-control documentation
- WHEN they configure or launch the bridge
- THEN the docs MUST state that MCP is an adapter over the daemon/session substrate and not a privileged TUI/controller backdoor
