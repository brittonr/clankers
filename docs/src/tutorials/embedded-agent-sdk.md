# Embedded Agent SDK

Clankers can be embedded as a Rust engine without importing the terminal app, daemon, provider discovery, session database, TUI, prompt assembly, plugin supervision, or built-in tool bundles. This guide defines supported crates, adapter contracts, validation policy, and recipe coverage for the productized embedded-agent path.

## Supported crate set

The supported embedding surface is the small reducer/host layer:

| Crate | Role | Supported entrypoints |
|---|---|---|
| `clankers-engine` | Pure turn-state reducer for accepted model/tool work | `EngineState`, `EngineInput`, `EnginePromptSubmission`, `EngineOutcome`, `EngineEffect`, `EngineModelRequest`, `EngineModelResponse`, `EngineTerminalFailure`, `EngineToolCall`, `reduce` |
| `clankers-engine-host` | Async effect interpreter that drives the reducer through caller adapters | `EngineRunSeed`, `HostAdapters`, `run_engine_turn`, `ModelHost`, `ModelHostOutcome`, `RetrySleeper`, `EngineEventSink`, `CancellationSource`, `UsageObserver`, stream accumulator types, runtime input helpers |
| `clankers-tool-host` | Provider-neutral tool execution contracts and output truncation | `ToolExecutor`, `ToolHostOutcome`, `ToolOutputAccumulator`, `ToolTruncationLimits`, `ToolTruncationMetadata`, `ToolCatalog`, `CapabilityChecker`, `ToolHook` |
| `clanker-message` | Shared message/content/tool/usage data types | `Content`, `StopReason`, `ToolDefinition`, `ThinkingConfig`, `Usage`, streaming/content delta types |
| `clankers-core` | Optional prompt-lifecycle reducer for hosts that want Clankers-style prompt/follow-up state before work reaches the engine | `CoreState`, `CoreInput`, `CoreOutcome`, `CoreEffect`, `reduce` |

The durable API inventory for these entrypoints lives in [`../generated/embedded-sdk-api.md`](../generated/embedded-sdk-api.md). If guide text names an SDK entrypoint, the checker added with this change must map it to an exported Rust item or a checked-in example path.

## Explicit exclusions

The generic embedding path does **not** require these Clankers shell concerns:

- daemon protocol or session sockets;
- terminal rendering, ratatui widgets, or keybindings;
- provider discovery, router daemon RPC, OAuth stores, or provider-shaped request/response structs;
- session database ownership, search indexes, or conversation storage;
- prompt assembly, skill loading, agent definitions, or system-prompt templates;
- plugin supervision, built-in tool bundles, Matrix, iroh/P2P, or process monitoring;
- Tokio runtime handles, network clients, shell-generated message IDs, wall-clock timestamps, or global singleton service lookup in generic SDK APIs.

Concrete providers, tools, storage, prompts, events, cancellation sources, and runtime choices belong at the embedder/application edge.

## Minimal turn flow

A host owns transcript conversion and then submits accepted work to the engine:

```rust,ignore
use clanker_message::Content;
use clankers_engine::{EngineInput, EnginePromptSubmission, EngineState, reduce};
use clankers_engine_host::{EngineRunSeed, HostAdapters, run_engine_turn};

let submission = EnginePromptSubmission {
    messages: vec![/* host-owned EngineMessage values */],
    model: "host-model".to_string(),
    system_prompt: "You are embedded".to_string(),
    max_tokens: None,
    temperature: None,
    thinking: None,
    tools: Vec::new(),
    no_cache: true,
    cache_ttl: None,
    session_id: "host-session".to_string(),
    model_request_slot_budget: 1,
};
let initial_state = EngineState::new();
let first_outcome = reduce(&initial_state, &EngineInput::submit_user_prompt(submission));
let report = run_engine_turn(
    EngineRunSeed::new(initial_state, first_outcome),
    HostAdapters {
        model: &mut model_host,
        tools: &mut tool_host,
        retry_sleeper: &mut retry_sleeper,
        event_sink: &mut event_sink,
        cancellation: &mut cancellation,
        usage_observer: &mut usage_observer,
    },
).await;
```

The checked-in consumer fixture under `examples/embedded-agent-sdk/` is the executable form of this sketch. It must stay outside the workspace crate graph and depend only on SDK crates plus application-owned executor/test helpers.

## Adapter contracts

`clankers-engine-host` depends on explicit interfaces. Hosts provide implementations; SDK crates do not instantiate Clankers runtime implementations.

| Adapter | Trait/type | Host responsibility | Positive path | Negative path |
|---|---|---|---|---|
| Model execution | `ModelHost` returns `ModelHostOutcome` | Convert `EngineModelRequest` into the host provider request, execute it, and convert output back to `EngineModelResponse` or stream events | Return `Completed` with `Content`, `StopReason`, and optional `Usage`, or `Streamed` provider-neutral events | Return `Failed { EngineTerminalFailure { retryable, status, message } }` for retryable and terminal provider failures |
| Tool execution | `ToolExecutor` returns `ToolHostOutcome` | Run application-owned tools and map host errors to plain tool outcomes | Return `Succeeded` with content/details | Return `ToolError`, `MissingTool`, `CapabilityDenied`, `Cancelled`, or `Truncated` |
| Retry sleeping | `RetrySleeper` | Apply host retry timing and cancellation policy | Return `Ok(())` when delay completes | Return `HostAdapterError` for sleeper failures; cancellation should finish promptly and let engine cancellation win |
| Event emission | `EngineEventSink` | Record or forward `EngineEvent` values to host UI/logging | Store `BusyChanged`, `Notice`, and `TurnFinished` diagnostics | Return `HostAdapterError` when host event plumbing fails; report keeps adapter diagnostics |
| Cancellation | `CancellationSource` | Tell the runner whether model/tool/retry work should stop | Return `false` while turn may continue | Return `true` and a host-owned reason; runner feeds `CancelTurn` before additional work |
| Usage observation | `UsageObserver` | Capture streaming usage deltas and final usage summaries | Accept `UsageObservationKind::StreamDelta` and `FinalSummary` | Return `HostAdapterError`; runner preserves diagnostic without requiring provider-specific usage types |
| Transcript conversion | Host code, not a runner trait | Convert persisted or shell-native messages into `EngineMessage` before submission | Map user/assistant/tool content into `EngineMessage` and `clanker_message::Content` | Reject unsupported shell-only message variants at the application edge; `clankers-engine` must not learn `AgentMessage` |

The example and validation bundle must exercise successful model responses, retryable model failures, non-retryable model failures, streamed deltas, successful tools, tool errors, missing tools, capability denial, cancellation, usage observations, and event-sink diagnostics.

## Adapter-only modular coupling rules

Keep generic crates dependency-inverted:

1. `clankers-engine` owns accepted turn policy only. It may use plain shared message/content data, but it must not import Clankers shell/runtime types.
2. `clankers-engine-host` owns effect interpretation and correlation plumbing only. It calls host traits; it must not discover providers, tools, prompts, sessions, daemons, plugins, network services, or runtime handles.
3. `clankers-tool-host` owns reusable tool outcome and truncation contracts only. It must not supervise plugins or call built-in tools.
4. `clanker-message` stays provider/router-neutral. Provider-native request shaping belongs in host adapters.
5. Application edge code may compose SDK crates with Clankers runtime crates, but that code is not part of the generic SDK surface.

`scripts/check-embedded-agent-sdk.sh` is the required acceptance command for these rules. It composes API inventory, docs freshness, example execution, feature/default checks, dependency denylist checks, source boundary checks, and focused Clankers parity rails.

## Feature and default policy

Current SDK crates are intended to work with their default features for the minimal embedding path:

- `clankers-engine`: no optional features; depends on `clanker-message` and `serde_json`.
- `clankers-engine-host`: no optional features; depends on `clankers-engine`, `clankers-tool-host`, `clanker-message`, `serde`, `serde_json`, and `thiserror`.
- `clankers-tool-host`: no optional features; depends on `clankers-engine`, `clanker-message`, `serde`, `serde_json`, and `thiserror`.
- `clanker-message`: default crate features are acceptable for embedding; it owns shared content/usage/message data, not application shells.
- `clankers-core`: optional for hosts that want prompt lifecycle/follow-up reduction before engine submission; not required by the minimal engine-host example.

The minimal embedding path must not require features that pull in daemon, TUI, provider discovery, database, prompt assembly, plugin supervision, built-in tools, Matrix, iroh, ratatui, or crossterm. Any future optional SDK feature must be documented here and validated by the feature/default-policy checker before it is advertised.

## Support, versioning, and migration policy

Clankers currently versions the SDK crates with the repository crate versions. Supported embedding entrypoints are the ones documented in this guide and classified in `docs/src/generated/embedded-sdk-api.md`.

Compatibility expectations:

- Supported entrypoints should not be removed, renamed, or semantically repurposed without an explicit migration note.
- Additions are allowed when they do not force forbidden shell/runtime dependencies into generic SDK crates.
- Unsupported/internal exported items may change without migration notes and must not be advertised as stable embedding API.
- Application-layer adapters that use `clankers-agent`, provider discovery, daemon, TUI, DB, prompts, or plugins are outside the generic SDK compatibility promise.

Migration notes for SDK changes belong in this guide under this section until a dedicated release-notes file exists. Each migration note should name the affected entrypoint, the replacement or adapter change, and the validation command that proves the new path.

## Validation checklist

Before claiming embedded SDK readiness, run:

```bash
scripts/check-embedded-agent-sdk.sh
```

That bundle must prove:

- documented entrypoints map to exported items or example paths;
- public API inventory is fresh;
- stale docs fail the checker;
- `examples/embedded-agent-sdk/` runs positive and negative adapter paths;
- example dependency graph excludes Clankers shell/runtime crates and UI/network crates listed in the OpenSpec change;
- feature/default policy matches manifests and a minimal example build;
- generic SDK crates reject provider/router, daemon/TUI, database, networking, timestamp, shell-generated ID, runtime-handle, provider-shaped request/response, hidden-global-service, and concrete Clankers runtime leakage;
- default `clankers-agent::Agent` still routes through the reusable host runner and preserves streaming, tool, retry, cancellation, usage, and terminal behavior.

## Migration notes

No embedded SDK migrations have been published yet. The first compatibility baseline is the API inventory generated by this change.
