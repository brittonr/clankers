## Context

Clankers has confirmation flows across TUI/daemon/session-control paths. For embedding, confirmation must become a host-service boundary so an application can show a native modal, consult policy, or deny all confirmations.

## Decisions

### Broker trait as policy boundary

**Choice:** Add a `ConfirmationBroker`-style interface that receives safe confirmation requests and returns decisions correlated by ids.

**Rationale:** Tool execution and session control can ask for permission without knowing whether the answer comes from TUI, daemon attach, MCP session control, or an embedding host.

### Deny by default when no broker is present

**Choice:** If a confirmation-required action reaches a runtime with no broker or an unavailable broker, it must fail closed.

**Rationale:** Embedding should never auto-approve dangerous actions because UI wiring is absent.

### Safe request summaries

**Choice:** Confirmation requests carry action kind, tool name, command/path summaries, risk labels, and redacted details rather than raw secret-bearing payloads by default.

**Rationale:** Hosts need enough context to ask a user or policy engine, while replay/debug metadata must remain safe.

## Risks / Trade-offs

- **Insufficient detail:** Over-redaction can make approvals unusable. Allow explicit visible fields while keeping replay metadata safe.
- **Parity drift:** Existing attach/TUI suppression and confirmation behavior is subtle. Add adapter parity tests.
- **Deadlocks:** Async broker calls must be cancellable/time-bounded and must not block event draining.
