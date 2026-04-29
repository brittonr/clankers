# Request Lifecycle

This page is the golden path for a normal user prompt. Keep it up to date when prompt execution, event translation, persistence, attach replay, or provider request shaping changes.

## One-shot map

```text
client input
  -> SessionCommand::Prompt or standalone Agent::run path
  -> SessionController / EventLoopRunner shell orchestration
  -> clankers-agent turn execution
  -> clankers-engine EngineModelRequest
  -> clankers-agent CompletionRequest adapter
  -> clankers-provider / clanker-router
  -> provider stream events
  -> AgentEvent broadcast
  -> controller persistence + DaemonEvent translation
  -> standalone TUI render or daemon broadcast
  -> attach client replay/live render
```

## Main roles

- `SessionController` is the transport-agnostic session shell. It accepts `SessionCommand` values and emits `DaemonEvent` values. It owns controller state, prompt busy state, loop/auto-test state, audit tracking, hooks, and optional `SessionManager` persistence.
- `Agent` owns model/tool execution for a turn. It emits `AgentEvent` values over a broadcast channel.
- `clankers-engine` owns the shell-independent engine request shape (`EngineMessage`, `EngineModelRequest`). It must not depend on provider/TUI/session shell types.
- `crates/clankers-agent/src/turn/execution.rs` is the shell adapter from `EngineModelRequest` to provider `CompletionRequest` and from provider stream events back to agent events.
- `clankers-provider` and `clanker-router` own provider-native transport, auth, routing, failover, and provider-specific request/stream normalization.
- `clankers-session` stores durable conversation history; the controller persists completed agent message batches on `AgentEvent::AgentEnd`.
- Daemon transports only frame and broadcast protocol events. They should not duplicate controller, agent, provider, or session state machines.

## Standalone prompt path

Standalone TUI/headless mode runs the agent in-process and renders agent events directly.

1. The event loop receives user input and starts an agent prompt.
2. `EventLoopRunner::process_agent_event` handles each `AgentEvent` in this order:
   - translate to a TUI event for real-time rendering,
   - feed the same event to `SessionController::feed_event`,
   - record usage,
   - dispatch plugin events,
   - persist tool-result side data.
3. `SessionController::feed_event` uses the same processing pipeline as daemon draining: audit, metrics, embedded prompt correlation, loop output accumulation, session persistence, `DaemonEvent` translation, and lifecycle hooks.
4. The standalone TUI should treat controller-produced daemon events as shell/control output, not as a second source of assistant transcript rendering.

## Daemon prompt path

Daemon mode wraps a `SessionController` in an actor process.

1. A client sends a framed `SessionCommand::Prompt` over the Unix-socket or QUIC bridge.
2. `run_agent_actor` receives the command and calls `SessionController::handle_command`.
3. `handle_command` validates controller state, starts prompt work, and drives the owned `Agent`.
4. The agent broadcasts `AgentEvent` values while the turn runs.
5. `socket_bridge::drain_and_broadcast` calls `SessionController::drain_events` and broadcasts the resulting `DaemonEvent` values to attached clients.
6. Plugin UI actions and subagent panel events are converted to daemon events at the bridge boundary after controller events are drained.

The daemon actor is responsible for multiplexing commands, actor signals, confirmation requests, plugin runtime events, and periodic event draining. It should not bypass `SessionController` for prompt lifecycle state.

## Provider request path

The provider boundary has two deliberate translations.

1. Agent messages are converted into engine-native `EngineMessage` values.
2. The engine emits `EngineModelRequest` values.
3. `completion_request_from_engine_request` converts an engine request into `clankers_provider::CompletionRequest`.
4. `stream_model_request` calls `Provider::complete` and concurrently collects provider stream events.
5. Provider stream events are normalized into `AgentEvent` updates and final assistant/tool-use messages.

Important invariants:

- Keep `EngineModelRequest` shell-native and provider-agnostic inside `clankers-engine`.
- Keep provider-specific request construction outside `clankers-engine`.
- Preserve `CompletionRequest.extra_params`, especially `_session_id` when a session id exists.
- Do not rebuild request messages by lossy `serde_json::to_value` conversions for routed provider backends; use the adapter path that preserves provider-native message content.
- Branch and compaction summaries are durable conversation context. If they are converted for a routed provider path, preserve them as user-visible text context rather than dropping them silently.

## Persistence and replay

Persistence is controller-owned.

- `SessionController::process_agent_event` calls `persist_event` before translating an `AgentEvent` into a `DaemonEvent`.
- `persist_event` appends messages to the `SessionManager` on `AgentEvent::AgentEnd`.
- `SessionManager::build_context` reconstructs context for resume and seed-message flows.
- Attach replay must preserve conversation-block ordering and metadata. A replayed block should finalize only when the original block completed, not simply because an assistant or tool-result message appeared.

If you change persistence or replay, check both restore paths:

- standalone resume/continue,
- daemon attach/recover, including keyed daemon sessions.

## Slash command and attach parity

Slash commands may execute locally in standalone mode, locally in attach mode before forwarding, or directly inside the daemon controller. Keep their observable state changes aligned.

Rules of thumb:

- If attach applies a local state update before forwarding a command, suppress only the matching daemon acknowledgement that would duplicate UI noise.
- Keep suppression narrow. Do not hide unrelated `SystemMessage` events.
- Update local and remote attach code together when parity behavior changes.
- Keep attach help text in lockstep with the actual routing table.

## High-risk seams to test

Add or update tests around these seams when touching them:

- `SessionController::handle_command(SessionCommand::Prompt { .. })` through a real prompt shell seam.
- `SessionController::feed_event` and `drain_events` parity for embedded vs daemon event processing.
- `_session_id` propagation into routed `CompletionRequest.extra_params` after session resume or slash-driven session swaps.
- Daemon attach replay ordering, conversation block metadata, and `HistoryEnd` finalization.
- Attach slash command parity for thinking level, disabled tools, model/role, and plugin fetches.
- Provider request-shape parity between `clankers-provider` and `clanker-router`.
- Runtime SSE/parser entrypoints, not only helper-level stream state machines.

## Ownership checklist for changes

Before merging a lifecycle change, answer these questions:

1. Which type is the source of truth at this boundary: `SessionCommand`, `AgentEvent`, `DaemonEvent`, `EngineModelRequest`, `CompletionRequest`, or `SessionEntry`?
2. Is the state transition owned by the controller, the agent, the engine, the provider, the transport, or the TUI?
3. Does standalone mode and daemon attach mode observe the same user-visible result?
4. Does resume/replay reconstruct the same context that the provider saw originally?
5. Did the change add or update a deterministic regression test at the boundary it touched?
