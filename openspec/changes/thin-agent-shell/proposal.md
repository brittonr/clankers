## Why

`clankers-agent` routes turns through `clankers-engine-host::run_engine_turn` via adapter structs (`AgentModelHost`, `AgentToolHost`, `AgentRetrySleeper`, `AgentEngineEventSink`, `AgentCancellationSource`, `AgentUsageObserver`), but those adapters still own substantial Clankers-specific logic inline: `AgentModelHost` does model-switch detection, `CompletionRequest` construction, provider streaming, `AssistantMessage` building, and message-history mutation; `AgentToolHost` does capability gating, hook dispatch, parallel execution, output truncation, turn-end bookkeeping, and `AgentMessage` history recording; `AgentEngineEventSink` does turn-lifecycle event emission. The `run_turn_loop` function itself is 13+ parameters, builds `TurnHostState` with shared mutable state behind `Arc<Mutex>`, and owns the `EngineState`→`EngineInput`→`reduce`→`EngineOutcome` prompt submission and the post-run error extraction. This makes `clankers-agent` a fat shell rather than a thin composition layer, defeats the reusability of the engine-host abstraction, and blocks embedding scenarios that want different provider wiring, message formats, or tool registries without forking agent internals.

## What Changes

- Extract the `AgentMessage`-level history mutation, `AssistantMessage` construction, turn-index bookkeeping, and batch tool-result accumulation out of the adapter structs into a standalone `TurnTranscript` type that adapters write to through a narrow interface, decoupling transcript shape from host trait implementations.
- Move `CompletionRequest` construction and provider streaming (`stream_model_request`) behind a dedicated `AgentModelAdapter` that implements `ModelHost` with no shared mutable state beyond the transcript writer, eliminating the `Arc<Mutex<TurnHostState>>` from model execution.
- Move capability gating, hook dispatch, parallel tool execution, and output truncation behind a dedicated `AgentToolAdapter` that implements `ToolExecutor` with the same transcript-writer pattern, removing direct `execute_tools_parallel` calls and inline truncation from the host trait.
- Collapse `run_turn_loop`'s 13+ parameter list into a `TurnLoopConfig` struct plus a `TurnLoopContext` struct that carries references to provider, tools, hooks, events, cancellation, and cost tracking, making the function signature a 3-argument composition point.
- Add deterministic boundary rails ensuring adapter structs contain no provider-streaming, no direct `clankers_provider::CompletionRequest` construction, no inline tool execution, and no `Arc<Mutex>` shared state.

## Capabilities

### New Capabilities
- `thin-agent-shell`: Contracts for the agent shell as a thin composition layer over engine-host adapters, with transcript isolation, adapter simplification, and parameter reduction.

### Modified Capabilities
- `embeddable-agent-engine`: The existing host-adapter contracts gain a tighter boundary — adapters must not own provider streaming, message construction, or shared mutable state.

## Impact

- `crates/clankers-agent/src/turn/mod.rs` (~3250 lines): Major restructuring of adapter structs, `TurnHostState`, and `run_turn_loop`.
- `crates/clankers-agent/src/turn/execution.rs` (~1167 lines): Functions move into adapter modules or the transcript type.
- `crates/clankers-controller/tests/fcis_shell_boundaries.rs`: New boundary rails for adapter purity.
- Existing `run_turn_loop` tests must adapt to the new parameter shape but preserve behavioral coverage.
