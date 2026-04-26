## Context

`clankers-agent/src/turn/mod.rs` (3250 lines) and `turn/execution.rs` (1167 lines) contain the full turn-loop implementation. The recent `extract-composable-engine-host` change moved the generic effect-interpretation loop into `clankers-engine-host::run_engine_turn`, and `clankers-agent` now implements `ModelHost`, `ToolExecutor`, `RetrySleeper`, `EngineEventSink`, `CancellationSource`, and `UsageObserver` as adapter structs. However, these adapters are fat: `AgentModelHost` owns model-switch detection, `CompletionRequest` construction, provider streaming, `AssistantMessage` building, and shared-state message-history mutation. `AgentToolHost` owns capability gating, hook dispatch, parallel execution, output truncation, and turn-end bookkeeping. All adapters share `Arc<Mutex<TurnHostState>>` for message history, turn indices, and batch accumulation.

`run_turn_loop` takes 13+ parameters including provider, controller tools, messages, config, event_tx, cancel token, cost tracker, model switch slot, hook pipeline, session ID, DB handle, capability gate, and user tool filter.

## Goals / Non-Goals

**Goals:**
- Make adapter structs pure delegation wrappers with no inline business logic.
- Isolate transcript/message-history state behind a narrow writer type.
- Collapse `run_turn_loop` parameters into structured config + context.
- Add boundary rails that prevent regression into fat adapters.

**Non-Goals:**
- Changing `clankers-engine` or `clankers-engine-host` trait signatures.
- Moving tool registration, capability gates, or hook infrastructure out of `clankers-agent`.
- Extracting session/transcript persistence (future change).
- Changing the provider streaming implementation itself.

## Decisions

### D1: TurnTranscript type with TurnTranscriptWriter handle

Introduce `TurnTranscript` as an `Arc<Mutex<...>>` internally but expose only `TurnTranscriptWriter` to adapters. The writer provides: `append_assistant(AssistantMessage)`, `append_tool_result(ToolResultMessage) -> Option<AgentEvent>`, `mark_turn_start(u32)`, `finish_turn() -> Option<AgentEvent>`, `active_model() -> String`, `set_active_model(String)`.

**Why not just pass `&mut Vec<AgentMessage>`:** The current adapters need concurrent access from model and tool paths; a narrow writer type constrains what adapters can do without exposing the full message vec. The internal `Arc<Mutex>` stays but the public surface is typed, not a generic lock.

**Alternative — channel-based transcript:** Would eliminate shared state entirely but adds message ordering complexity and makes synchronous turn-end detection harder. Not worth it for this change.

### D2: AgentModelAdapter delegates to execution module functions

The model adapter struct holds: provider ref, event_tx ref, cancel token, model_switch_slot ref, transcript writer. Its `execute_model` impl calls `check_model_switch(...)` → `completion_request_from_engine_request(...)` → `stream_model_request(...)` → builds `AssistantMessage` → writes to transcript. Each of those is already a function in `execution.rs`; the adapter just sequences them.

**Why not a trait object for model execution:** The provider is already behind `dyn Provider`. Adding another trait indirection would just move the delegation one step further without simplifying the adapter.

### D3: AgentToolAdapter delegates to execution module functions

The tool adapter struct holds: controller tools ref, event_tx ref, cancel token, hook pipeline, session ID, DB, capability gate, user tool filter, output truncation config, transcript writer. Its `execute_tool` impl calls `execute_tools_parallel(...)` → `apply_output_truncation(...)` → `tool_result_message_to_host_outcome(...)` → writes to transcript.

### D4: TurnLoopConfig and TurnLoopContext structs

```rust
pub struct TurnLoopConfig {
    pub model: String,
    pub system_prompt: String,
    pub max_tokens: Option<usize>,
    pub temperature: Option<f64>,
    pub thinking: Option<ThinkingConfig>,
    pub model_request_slot_budget: u32,
    pub output_truncation: OutputTruncationConfig,
    pub no_cache: bool,
    pub cache_ttl: Option<String>,
}

pub struct TurnLoopContext<'a> {
    pub provider: &'a dyn Provider,
    pub controller_tools: &'a HashMap<String, Arc<dyn Tool>>,
    pub event_tx: &'a broadcast::Sender<AgentEvent>,
    pub cancel: CancellationToken,
    pub cost_tracker: Option<&'a Arc<CostTracker>>,
    pub model_switch_slot: Option<&'a ModelSwitchSlot>,
    pub hook_pipeline: Option<Arc<HookPipeline>>,
    pub session_id: &'a str,
    pub db: Option<Db>,
    pub capability_gate: Option<&'a Arc<dyn CapabilityGate>>,
    pub user_tool_filter: Option<&'a Vec<String>>,
}
```

`run_turn_loop(config: &TurnLoopConfig, ctx: TurnLoopContext<'_>, messages: &mut Vec<AgentMessage>) -> Result<()>`

**Why not a builder:** The function is called from one place in the agent loop. A builder adds ceremony without ergonomic benefit.

### D5: Boundary rails via source parsing

Extend `fcis_shell_boundaries.rs` with a test that parses `impl ModelHost for` and `impl ToolExecutor for` blocks in `turn/mod.rs` and asserts they do not contain `CompletionRequest`, `stream_model_request`, `execute_tools_parallel`, `StreamEvent`, `Arc<Mutex<TurnHostState>>`, or `SharedTurnHostState` as direct references. This is the same syn-based approach used for existing FCIS boundaries.

## Risks / Trade-offs

- [Churn in test code] The 5+ existing `run_turn_loop` tests call the 13-parameter function directly. They must adapt to the new struct-based signature. → Mitigation: mechanical refactor, behavioral coverage unchanged.
- [TurnTranscript adds indirection] Adapters now go through a writer instead of direct vec mutation. → Mitigation: the writer is a thin layer; no runtime cost beyond the lock that already exists.
- [Boundary rails can be brittle] Syn-based source parsing can break on formatting changes. → Mitigation: existing FCIS rails have been stable; same pattern.
