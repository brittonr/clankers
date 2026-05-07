## ADDED Requirements

### Requirement: Host confirmation broker [r[embeddable-confirmation-broker.interface]]

The system MUST provide a host-facing confirmation broker interface for confirmation-required actions so embedding hosts can approve, deny, timeout, or cancel requests without using TUI state, daemon frames, slash commands, ACP/MCP internals, or raw terminal input as the primary approval API.

#### Scenario: broker approves an action [r[embeddable-confirmation-broker.interface.approve]]

- GIVEN a tool or session action requires confirmation and the runtime has a broker configured
- WHEN the broker returns an approval for the matching confirmation id before expiry or cancellation
- THEN the action may proceed through the normal tool/session policy path
- THEN the resulting events and metadata record safe confirmation id, action kind, decision, and status

#### Scenario: broker is absent or unavailable [r[embeddable-confirmation-broker.interface.fail-closed]]

- GIVEN a tool or session action requires confirmation and no broker is configured or the broker returns unavailable
- WHEN the action reaches the confirmation boundary
- THEN the action is denied or rejected before side effects occur
- THEN the host receives an explicit fail-closed event/error explaining that confirmation was unavailable

### Requirement: Confirmation request safety [r[embeddable-confirmation-broker.safe-requests]]

Confirmation requests and replay/debug metadata MUST carry safe action summaries and MUST NOT leak raw credentials, headers, environment values, hidden prompt context, or unredacted secret-like tool payloads.

#### Scenario: request summary is usable and redacted [r[embeddable-confirmation-broker.safe-requests.redacted]]

- GIVEN a shell, filesystem, browser, plugin, MCP, Matrix, or gateway action asks for confirmation
- WHEN the request is sent to a host broker
- THEN the request includes action kind, tool/source label, risk label, visible user-facing summary, confirmation id, and expiry/cancel metadata
- THEN replay/debug metadata contains only safe ids, labels, counts, statuses, and error classes unless the host explicitly includes a visible prompt for the user

### Requirement: Confirmation adapter parity [r[embeddable-confirmation-broker.adapter-parity]]

The system MUST verify that TUI, daemon/attach, MCP/ACP, and embeddable hosts route confirmation decisions through the same policy substrate for covered actions.

#### Scenario: no bypass by transport adapter [r[embeddable-confirmation-broker.adapter-parity.no-bypass]]

- GIVEN a confirmation-required action is initiated through TUI, daemon attach, MCP session control, ACP, or the embeddable runtime
- WHEN the action is approved or denied
- THEN the decision is applied through the shared confirmation broker/policy substrate
- THEN no adapter directly mutates private state or injects raw approval input to bypass confirmation policy
