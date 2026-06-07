# Design: Trait Seam Refactor Roadmap

## Trait selection rule

A trait seam is justified only when a caller needs to depend on a stable behavior boundary while multiple implementations or test doubles vary behind it. Do not introduce traits for passive DTOs, one-off enum labels, or simple constructor helpers.

Each accepted seam must name:

- the functional-core owner of behavior and invariants;
- the imperative-shell adapter that touches filesystem, network, process, clock, or runtime state;
- the DTOs that cross the seam;
- the focused tests or rails that prove parity before and after the move.

## Candidate seams

### Plugin runtime trait

`PluginManager` currently owns Extism instances, stdio supervisor state, live registrations, and host-event queues while branching on `PluginKind`. Introduce a runtime-owned trait only after the shape can absorb Extism and stdio without moving manifest validation or plugin summary projection into the runtime implementations.

A likely shape is `PluginRuntime` with operations for load/start, stop, reload, call/tool invocation, live inventory, event drain, and state projection. `PluginManager` remains the registry/orchestrator; runtime implementations own runtime-specific handles.

### OAuth provider flow trait

`OAuthFlow` currently dispatches Anthropic and OpenAI Codex with enum matches. A provider-flow trait is appropriate before adding another OAuth provider. The trait should cover auth URL construction, code exchange, refresh, provider name, and optional account-identity derivation. Credential store helpers remain provider-scoped and shared.

### Framed session transport trait

Unix socket attach/control and QUIC attach/control repeat framed read/write and handshake choreography. A transport trait should abstract I/O and reconnect capability while leaving wire DTO construction in the existing controller transport conversion owners.

### Session format trait

Session JSONL and Automerge paths are selected by filename extension. A `SessionFormat` or `SessionStore` trait can isolate read, append, summary, and migration behavior so callers do not grow format-specific branches as Automerge becomes the primary path.

### Process-job shell port traits

Process-job services already have typed service/backend boundaries. The next trait candidates are smaller shell ports: command execution and clock/time. They should reduce duplicated pueue/systemd runner code and make durable reconciliation tests deterministic without moving backend policy back into the root tool.

## Non-goals

- Do not traitify data-only receipt, status, manifest, or request DTOs.
- Do not hide policy owner drift behind `dyn Trait`; source rails must still identify where each concern lives.
- Do not combine unrelated seams into one mega trait.
