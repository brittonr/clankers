## ADDED Requirements

### Requirement: Agent turn adapters MUST be pure delegation wrappers

Each `clankers-agent` host-trait adapter (`ModelHost`, `ToolExecutor`, `RetrySleeper`, `EngineEventSink`, `CancellationSource`, `UsageObserver`) MUST delegate to purpose-built modules and MUST NOT contain inline provider streaming, `CompletionRequest` construction, tool execution orchestration, or shared mutable turn state.
r[thin-agent-shell.adapters-are-pure-delegation]

#### Scenario: model adapter delegates to a conversion module and provider caller
r[thin-agent-shell.model-adapter-delegates-to-modules]

- **WHEN** the engine-host runner invokes `ModelHost::execute_model` on the agent's model adapter
- **THEN** the adapter calls a conversion function to build a `CompletionRequest` from the `EngineModelRequest`
- **THEN** the adapter calls a streaming function to execute the provider request
- **THEN** the adapter does not own streaming loop iteration, SSE parsing, content-block accumulation, or provider error mapping inline

#### Scenario: tool adapter delegates to execution and truncation modules
r[thin-agent-shell.tool-adapter-delegates-to-modules]

- **WHEN** the engine-host runner invokes `ToolExecutor::execute_tool` on the agent's tool adapter
- **THEN** the adapter calls the tool execution module for capability gating, hook dispatch, and parallel execution
- **THEN** the adapter calls the truncation module for output limits
- **THEN** the adapter does not own inline capability gate checks, hook pipeline calls, or truncation arithmetic

### Requirement: Turn transcript state MUST be isolated from adapter structs

The agent shell MUST own a `TurnTranscript` (or equivalent) type that accumulates `AgentMessage` history, assistant snapshots, turn-index bookkeeping, and batch tool-result tracking. Adapter structs MUST write to this transcript through a narrow writer interface rather than holding `Arc<Mutex<TurnHostState>>`.
r[thin-agent-shell.transcript-isolated-from-adapters]

#### Scenario: transcript writer replaces shared mutable state in adapters
r[thin-agent-shell.transcript-writer-replaces-arc-mutex]

- **WHEN** multiple adapters need to record messages or read turn state during a turn
- **THEN** each adapter holds a `TurnTranscriptWriter` (or equivalent handle) that provides append-only message recording and turn-index queries
- **THEN** no adapter struct holds an `Arc<Mutex<TurnHostState>>` or equivalent shared lock over heterogeneous turn state

#### Scenario: transcript accumulates messages independently of adapter lifetime
r[thin-agent-shell.transcript-independent-of-adapter-lifetime]

- **WHEN** the turn completes and adapters are dropped
- **THEN** the transcript retains the full message sequence, cumulative usage, and final turn index
- **THEN** `run_turn_loop` reads the transcript to produce its return value without re-querying adapter state

### Requirement: run_turn_loop MUST accept structured configuration and context

The `run_turn_loop` entry point MUST accept a structured configuration type and a structured context type instead of 13+ positional parameters.
r[thin-agent-shell.structured-turn-loop-params]

#### Scenario: configuration and context replace positional parameters
r[thin-agent-shell.config-context-replace-positional]

- **WHEN** agent code calls `run_turn_loop`
- **THEN** the function signature accepts at most 3 positional arguments: a config struct, a context struct carrying references to provider/tools/hooks/events/cancellation/cost-tracking, and the mutable message history
- **THEN** adding a new optional concern (e.g., a new hook type or tracking handle) requires adding a field to the context struct, not changing the function signature

### Requirement: Boundary rails MUST enforce adapter purity

Deterministic boundary tests MUST verify that adapter structs in `clankers-agent/src/turn/` contain no direct provider-streaming calls, no `CompletionRequest` construction, no inline tool execution dispatch, and no `Arc<Mutex>` shared-state patterns.
r[thin-agent-shell.boundary-rails-enforce-adapter-purity]

#### Scenario: FCIS rails reject provider streaming in adapter structs
r[thin-agent-shell.fcis-rejects-provider-streaming-in-adapters]

- **WHEN** the boundary test parses adapter struct `impl` blocks in `crates/clankers-agent/src/turn/mod.rs`
- **THEN** no adapter `impl` block references `stream_model_request`, `CompletionRequest`, `StreamEvent`, or provider-streaming types directly
- **THEN** no adapter struct field has type `Arc<Mutex<TurnHostState>>` or `SharedTurnHostState`

#### Scenario: FCIS rails reject inline tool execution in adapter structs
r[thin-agent-shell.fcis-rejects-inline-tool-execution-in-adapters]

- **WHEN** the boundary test parses adapter struct `impl` blocks in `crates/clankers-agent/src/turn/mod.rs`
- **THEN** no adapter `impl` block calls `execute_tools_parallel` directly
- **THEN** no adapter `impl` block performs inline capability gate checks or hook pipeline dispatch
