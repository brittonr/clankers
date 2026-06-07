# Design: Codify Neutral Port Boundaries

## Rule

Reusable policy modules may own pure DTOs, reducers, and traits. They may not own concrete IO implementations. Concrete adapters live at application edges and are injected through typed ports.

Preferred flow:

```text
DomainInput -> DomainDecision/CoreEffect -> PortTrait -> ShellAdapter -> Receipt/Event
```

The adapter may call providers, tools, storage, hooks, plugins, or config readers. The reusable policy returns typed decisions/receipts and never performs ambient discovery.

## Verification

Rails should inventory public port traits, request/decision/receipt DTOs, and concrete adapter implementations separately. A failure should explain whether the fix is to move DTOs down, move IO up, or add an explicit adapter exception.
