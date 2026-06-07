# Design: Trait Seam Refactor Roadmap

## Trait selection rule

A trait seam is justified only when a caller needs to depend on a stable behavior boundary while multiple implementations or test doubles vary behind it. Do not introduce traits for passive DTOs, one-off enum labels, or simple constructor helpers.

Each accepted seam must name:

- the functional-core owner of behavior and invariants;
- the imperative-shell adapter that touches filesystem, network, process, clock, or runtime state;
- the DTOs that cross the seam;
- the focused tests or rails that prove parity before and after the move.

## Drain decisions

| Candidate | Decision | Behavior owner | Adapter boundary | DTOs crossing seam | Verification rail |
|-----------|----------|----------------|------------------|--------------------|-------------------|
| Plugin runtime lifecycle | Traitify now | `crates/clankers-plugin::PluginManager` remains registry/orchestrator; `runtime::PluginRuntimeLifecycle` owns Extism vs stdio lifecycle actions | Extism WASM instance map and stdio supervisor/live-state bags stay runtime-specific shell state under `ExtismRuntimeState` / `StdioRuntimeState` | `PluginInfo`, `PluginState`, `PluginKind`, stdio registered tool/event DTOs | `cargo test -p clankers-plugin --lib plugin_runtime_dispatch_kit` |
| OAuth provider flow | Traitify now | `crates/clankers-provider::auth::OAuthFlow` remains provider selection surface; `OAuthProviderFlow` owns provider-specific URL/exchange/refresh behavior | Anthropic router OAuth functions and OpenAI Codex reqwest token calls remain provider adapters | provider name, PKCE verifier, auth URL, OAuth credentials, Codex account claim | `cargo test -p clankers-provider --lib oauth_flow`; `cargo test -p clankers-provider --lib openai_codex_auth` |
| Framed session transport | Use existing generic frame seam and remove QUIC duplicate framing | `clankers-protocol::frame` owns length-prefixed JSON policy; controller `transport_convert` remains wire DTO projection owner | Unix sockets and QUIC `QuicBiStream` both adapt to `AsyncRead + AsyncWrite`; attach/control code calls shared `frame::{read_frame,write_frame}` | `DaemonRequest`, `AttachResponse`, `ControlResponse`, `DaemonEvent`, `SessionCommand` | local/remote reconnect tests plus `clankers-controller` FCIS transport-construction rail |
| Session format/store | Traitify now | `crates/clankers-session::session_format` owns JSONL vs Automerge behavior | JSONL file reads and Automerge document load/save/migration remain format adapters | `SessionEntry`, `SessionSummary`, `HeaderEntry`, Automerge document path | store tests plus JSONL→Automerge migration test |
| Process-job shell command runner | Traitify now as a narrow shell port | Existing `ProcessJobService`/backend types keep policy; `ProcessJobCommandRunner` owns child-command execution | Tokio `Command` execution is shared below pueue/systemd runners; backend-specific request/status parsing remains in each backend | program name, CLI args, stdout/error string, typed receipts | pueue/systemd backend seam tests plus native durable/retention/notification tests |
| Passive receipt/status/request DTOs | Keep as DTOs | Runtime process-job and plugin/session DTO owners remain unchanged | No shell adapter | Receipt, status, request, manifest structs | Covered by the same focused rails; no trait introduced only for style |
| Single-implementation helpers | Defer | Current owning modules stay direct until a second implementation or deterministic fake is needed | No new adapter | Helper-specific values only | No validation rail beyond compile/check |

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
