## Verification Summary

This change is ready for implementation only after the contract-surface cleanup has landed or equivalent dependency rails are green. Implementation is done when Clankers runtime flows use the reusable host runner, parity tests prove behavior did not move backward into `clankers-agent`, and static rails prove the new host crates stay dependency-light.

## Context

`clankers-engine` owns reducer decisions, but `clankers-agent::turn::run_turn_loop` remains the only reusable async shell that actually executes a full turn. It mixes effect interpretation, provider streaming, stream accumulation, tool execution, hooks, DB/capability plumbing, usage tracking, cancellation, retry sleeping, model switching, and event emission.

Embedders need smaller layers: they should compose an engine runner with their own model client, tool executor, event sink, storage, and policy adapters.

Prerequisites are the archived changes `decouple-llm-contract-surface` and `separate-engine-core-composition`; implementation readiness is not claimed by design text alone. Before implementation starts, run `./scripts/check-llm-contract-boundary.sh`, `cargo test -p clankers-controller --test fcis_shell_boundaries`, and `openspec validate extract-composable-engine-host --strict`; those command outputs are the acceptance evidence for the cleanup/rail precondition.

## Goals / Non-Goals

**Goals**

- Extract reusable async engine-effect interpretation into `clankers-engine-host`.
- Define trait seams for model execution, tool execution, retry sleeping, event emission, cancellation, and usage observation. [r:embeddable-agent-engine.host-runner-traits]
- Extract reusable tool execution/catalog/result accumulation into `clankers-tool-host`. [r:embeddable-agent-engine.tool-host-catalog]
- Extract deterministic stream accumulation into `clankers-engine-host::stream` with positive, negative, and parser/adapter seam tests. [r:embeddable-agent-engine.reusable-stream-accumulator]
- Preserve existing Clankers `Agent` behavior through adapters. [r:embeddable-agent-engine.agent-default-assembly]

**Non-Goals**

- Do not move retry/backoff, continuation-budget, token-limit, terminal stop, or tool-continuation policy out of `clankers-engine`.
- Do not move provider backends or Clankers provider discovery into the engine host.
- Do not move plugin runtime supervision into `clankers-engine-host` or `clankers-tool-host`.
- Do not move system prompt assembly, config discovery, session DB ownership, daemon protocol, or TUI rendering.
- Do not remove the existing Clankers `Agent` public assembly yet.

## Crate Boundaries

### `clankers-engine-host`

Owns the async host runner and stream accumulator. It depends on `clankers-tool-host` for tool outcome types and tool execution traits rather than redefining tool outcomes locally. It may also depend on `clankers-engine`, `clanker-message`, `serde_json`, `async-trait` or equivalent trait support, and tiny utility crates needed for explicit cancellation/sleep abstractions.

It must not depend on daemon protocol, TUI crates, session DB, built-in tool bundles, plugin runtime supervision, system prompt assembly, Clankers provider discovery, or model-selection policy. [r:embeddable-agent-engine.host-crate-boundary-rails]

### `clankers-tool-host`

Owns reusable tool catalog/executor/result accumulation contracts. It may depend on engine-native tool call/correlation data, `clanker-message`, `serde_json`, and truncation code owned inside `clankers-tool-host` itself. No external Clankers truncation helper crate/module is allowed unless it is first extracted as a reusable dependency in a separate change.

It must not depend on daemon protocol, TUI crates, session DB, built-in tool bundles, plugin runtime supervision, Clankers provider discovery, model-selection policy, system prompt assembly, or engine reducer internals beyond this exact allowed list: `EngineToolCall`, `EngineCorrelationId`, `Content`, and plain tool request/result content structs owned by `clankers-tool-host`. It must not use `EngineToolRequest`, `EngineToolResult`, `EngineInput`, `EngineEffect`, `EngineOutcome`, or reducer state. [r:embeddable-agent-engine.host-crate-boundary-rails]

`clankers-agent` remains the product assembly that adapts Clankers provider/router, built-in tools, hooks, DB, capability gate, plugin tools, model switch slot, event bus, settings, and cancellation token to these reusable crates. [r:embeddable-agent-engine.agent-default-assembly]

## Host Runner Contract

The runner start API is engine-native and async: `async fn run_engine_turn(seed: EngineRunSeed, hosts: HostAdapters) -> EngineRunReport` (or an equivalent `Future<Output = EngineRunReport>` trait method). `EngineRunSeed` contains the initial `EngineState` plus the first accepted `EngineOutcome` already produced by `clankers-engine` public start helpers. `clankers-engine-host` does not accept or construct `EngineInput::SubmitUserPrompt`; Clankers controller composition or an embedder-owned engine helper call produces the seed before host execution. The host runner only constructs correlated model/tool/retry/cancel feedback inputs after the engine has accepted the prompt. `EngineRunReport` returns final `EngineState`, the last `EngineOutcome` (terminal success/failure/cancellation or explicit reducer rejection), ordered observed events, usage observations, and adapter diagnostics; session persistence remains shell-owned.

Host traits:

- `ModelHost`: executes `EngineModelRequest` and returns model success/failure as data, without owning retry policy.
- `ToolExecutor`: executes `EngineToolCall` values and returns success/failure content through `clankers-tool-host` outcomes.
- `RetrySleeper`: sleeps for engine-emitted retry delays only; it does not calculate backoff or retry budgets.
- `EngineEventSink`: observes engine events and adapter events in order. It is best-effort: sink failures are recorded in `EngineRunReport.adapter_diagnostics` and do not change reducer feedback or terminal behavior.
- `CancellationSource`: reports cancellation; the runner converts it into `EngineInput::CancelTurn` only after the engine has accepted the prompt and work is in a model/tool/retry phase.
- `UsageObserver`: receives usage deltas in stream arrival order when `HostStreamEvent::Usage` arrives and receives one final usage summary after model completion. It is best-effort: observer failures are recorded in `EngineRunReport.adapter_diagnostics` and do not affect reducer state, retryability, or terminal behavior.

Correlation rules [r:embeddable-agent-engine.host-feedback-construction-seam]:

- Every model response/failure must feed `EngineInput::ModelCompleted` / `ModelFailed` with the request ID emitted by the engine.
- Every tool result/failure must feed `EngineInput::ToolCompleted` / `ToolFailed` with the call ID emitted by the engine.
- Retry-ready feedback is emitted only after an engine `ScheduleRetry` effect and the injected sleeper returns.
- Wrong-phase, duplicate, mismatched, or post-terminal feedback remains an engine rejection; host code must not terminalize locally. [r:embeddable-agent-engine.host-runner-traits]


Streaming/event ordering:

- Provider-native bytes/events are normalized in Clankers provider/agent adapter code before crossing into `clankers-engine-host`.
- `ModelHost::execute_streaming(request, stream_sink)` sends provider-neutral `HostStreamEvent` values into a host-owned `StreamSink` callback supplied by the runner. `HostStreamEvent` is defined in `clankers-engine-host::stream` using only `clanker-message`/engine-neutral content, usage, model-name, stop-reason, provider-error, and tool-JSON-delta data.
- The runner feeds `HostStreamEvent` values through `clankers-engine-host::stream` in arrival order and emits normalized stream events to `EngineEventSink` before feeding the final model completion back to the reducer.
- Accumulator errors map to correlated `EngineInput::ModelFailed`: malformed tool JSON, non-object tool JSON, missing block starts, duplicate indexes, and late deltas become non-retryable model failures with deterministic diagnostics; provider error events become retryable or non-retryable model failures according to the provider-error retryability flag; usage-only and empty-stop normalized results remain successful completions. Adapter diagnostics may mirror the error, but reducer feedback is always the correlated model failure path.
- Provider/TUI/daemon-specific event shaping stays in Clankers adapter sinks; the generic runner only preserves ordering between normalized stream deltas, tool events, retry events, cancellation events, usage observations, and terminal engine events.
- A non-streaming `ModelHost` adapter is represented as one final completion chunk, so ordering tests cover both paths.


Effect scheduling:

- The runner processes each `EngineOutcome.effects` list in emitted order for observer/event emission.
- Tool-call effects from the same outcome execute sequentially in emitted order for this change; concurrent tool scheduling is deferred to a future spec.
- Model request, retry-sleep, cancellation, and terminal effects are single-flight and are never executed concurrently with another model/retry/terminal effect.
- Cancellation races the currently awaited effect, sends one turn cancellation, ignores the late result from that effect, and stops before launching any later effect.
- Parity tests assert tool-call event order, hook order, cancellation-before-next-tool behavior, and terminal event ordering.

In-flight cancellation:

- The runner races `CancellationSource::cancelled()` against awaited model, tool, and retry-sleep futures.
- If cancellation wins, the runner sends `EngineInput::CancelTurn` once with the current engine correlation ID, emits cancellation observation, and ignores any late model/tool/sleep result for that effect.
- Adapters may also abort their underlying work, but reducer correctness cannot depend on successful abort. The production runner drops late results after cancellation once it has sent `CancelTurn`; separate engine/host harness tests inject duplicate/post-terminal feedback directly into the reducer seam to prove rejection behavior without requiring the production runner to forward ignored late adapter results.

Policy boundary:

- The runner interprets effects; it does not choose retry counts, retry delays, continuation budget, token-limit terminalization, stop reason terminal policy, or tool-continuation decisions. [r:embeddable-agent-engine.no-duplicated-runner-policy]

## Tool Host Contract

`clankers-tool-host` exposes:

- `ToolCatalog`: lookup by tool name and metadata listing.
- `ToolExecutor`: executes a correlated engine tool call.
- `ToolHook`: generic before/after hook trait with plain data, not `clankers_hooks::HookPipeline`.
- `CapabilityChecker`: optional pre-execution capability seam returning allow/deny data. Clankers adapters plug existing capability policy here; generic `clankers-tool-host` only transports the result.
- `ToolOutputAccumulator`: accumulates result chunks and applies truncation from adapter-supplied `ToolTruncationLimits { max_bytes, max_lines }`. Counting uses UTF-8 byte length and line count based on newline boundaries, truncates only at valid UTF-8 boundaries, preserves original byte/line counts, and emits deterministic metadata `{ original_bytes, original_lines, truncated_bytes, truncated_lines }`. Clankers adapters pass the existing Clankers tool-output limits so shell-visible truncation behavior remains unchanged.

Outcome variants exposed by `clankers-tool-host` are plain data; `clankers-tool-host` does not import or construct `EngineInput`. The outcome-to-feedback mapping below is implemented only in `clankers-engine-host::runner` after the tool-host call returns:

| Tool-host outcome | Engine-host feedback | Notes |
| --- | --- | --- |
| `Succeeded { content, details }` | `EngineInput::ToolCompleted` | Content passed unchanged. |
| `Truncated { content, metadata: ToolTruncationMetadata { original_bytes, original_lines, truncated_bytes, truncated_lines } }` | `EngineInput::ToolCompleted` | Truncated content plus metadata; truncation is not engine failure. |
| `ToolError { content, details, message }` | `EngineInput::ToolFailed` | Error content/message preserved. |
| `MissingTool { name }` | `EngineInput::ToolFailed` | Deterministic missing-tool error. |
| `CapabilityDenied { name, reason }` | `EngineInput::ToolFailed` | Deterministic capability-denied error. |
| `Cancelled { name }` | `EngineInput::CancelTurn` when turn cancellation wins, otherwise `EngineInput::ToolFailed` with cancelled tool error | Turn-level cancellation wins over a tool-local cancelled result. |

Built-in tools, WASM plugins, and stdio plugins adapt into this surface from `clankers-agent` / plugin crates; plugin supervision does not move into the generic host crates. [r:embeddable-agent-engine.plugin-tool-adapter]

## Stream Accumulator Contract

`clankers-engine-host::stream` owns deterministic folding from provider stream events into canonical assistant content, usage, model name, and stop reason. Event mirroring to TUI/daemon stays in adapters. [r:embeddable-agent-engine.stream-folding-positive]

Malformed-input matrix [r:embeddable-agent-engine.stream-folding-negative]:

| Input | Outcome |
| --- | --- |
| malformed tool JSON delta | explicit `MalformedToolJson` error with block index |
| non-object tool JSON | explicit `NonObjectToolJson` error with block index |
| delta before content-block start | explicit `MissingContentBlockStart` error |
| duplicate block index start | explicit `DuplicateContentBlockIndex` error |
| late delta after block stop | explicit `LateContentDelta` error |
| provider error event | explicit provider error result preserving status/retryability |
| usage delta without content | normalized usage-only update |
| message stop without content | normalized empty assistant response with stop reason |

Verification must include accumulator unit fixtures and at least one parser/adapter seam test that feeds raw provider stream bytes or provider-native events through the real stream-normalization entrypoint before asserting accumulator output.

## Verification Matrix

| Requirement ID | Verification |
| --- | --- |
| r[embeddable-agent-engine.composable-host-contract] | Covered by `host-runner-traits`, `host-feedback-construction-seam`, and `host-crate-boundary-rails` rows. |
| r[embeddable-agent-engine.host-runner-traits] | Fake model/tool/sleep/event/cancel adapters drive model success/failure, tool success/failure, retry scheduling, cancellation, terminal outcomes, event-sink failure diagnostics, wrong-phase feedback, duplicate feedback, mismatched IDs, post-terminal feedback through the reducer seam, ignored late adapter results after cancellation, and prove the host runner does not terminalize locally or alter reducer feedback on adapter observer failures. |
| r[embeddable-agent-engine.agent-default-assembly] | Runtime parity tests prove `clankers-agent::Agent` remains the default assembly over the host runner, with unchanged public prompt APIs, event ordering, tools, hooks, usage, model switching, and cancellation behavior; coverage includes the standalone interactive path, daemon session path, and attach replay/attached prompt path, or a shared `Agent` seam test plus one smoke assertion per entrypoint proving each flow routes through that seam. Usage assertions cover stream delta ordering, final usage summary delivery, and `EngineRunReport` usage observations. |
| r[embeddable-agent-engine.reusable-tool-host] | Covered by `tool-host-catalog` and `plugin-tool-adapter` rows. |
| r[embeddable-agent-engine.tool-host-catalog] | Unit tests cover every tool-host outcome, result accumulation, UTF-8-safe byte/line truncation with existing Clankers limits, missing tool, capability denial, cancellation, and hook invocation order. |
| r[embeddable-agent-engine.tool-host-outcome-verification] | Tests cover missing tools, capability denial, tool cancellation, output truncation, result accumulation, hook ordering, and usage-observer failure recorded as adapter diagnostics without changing reducer feedback or terminal behavior. |
| r[embeddable-agent-engine.plugin-tool-adapter] | Adapter tests prove built-in/WASM/stdio tool wrappers implement the same tool-host executor seam without moving plugin supervision. |
| r[embeddable-agent-engine.reusable-stream-accumulator] | Covered by `stream-folding-positive` and `stream-folding-negative` rows. |
| r[embeddable-agent-engine.stream-folding-positive] | Positive stream accumulator fixtures cover text, thinking, tool JSON, usage deltas, model name, and stop reason. |
| r[embeddable-agent-engine.stream-folding-negative] | Negative fixtures cover the malformed-input matrix plus one real parser/adapter seam test; host-runner tests prove accumulator/provider-error results become correlated `EngineInput::ModelFailed` with request ID and retryability preserved. |
| r[embeddable-agent-engine.host-extraction-rails] | Covered by `no-duplicated-runner-policy`, `host-crate-boundary-rails`, and `host-adapter-parity` rows. |
| r[embeddable-agent-engine.adapter-parity-rails] | Covered by `adapter-rail`, `host-adapter-parity`, and runtime parity rows for retry, budget, token-limit, cancellation, and terminal behavior. |
| r[embeddable-agent-engine.adapter-rail] | Source rails cover `crates/clankers-agent/src/{lib.rs,turn/mod.rs,turn/execution.rs}`: `turn` delegates retry/terminal/cancel effect driving to `clankers-engine-host`, while `lib.rs` retains only named adapter constants that pass existing budgets into engine/host configuration. |
| r[embeddable-agent-engine.no-dormant-core-state] | Existing engine-state inventory remains in force, and cancellation ownership is verified by the `cancellation-phase-ownership` row. |
| r[embeddable-agent-engine.core-engine-boundary-rails] | Covered by `host-feedback-construction-seam`, `no-duplicated-runner-policy`, `engine-excludes-core-dependency`, `agent-core-type-rail`, `composition-tests`, and static rail inventories. |
| r[embeddable-agent-engine.engine-excludes-core-dependency] | Cargo-tree rail continues to prove `clankers-engine` excludes `clankers-core` while host extraction keeps composition in adapter/host code. |
| r[embeddable-agent-engine.agent-core-type-rail] | FCIS source rail inventories non-test `crates/clankers-agent/src/**` and rejects core lifecycle types outside controller-owned adapter seams. |
| r[embeddable-agent-engine.composition-tests] | Adapter composition tests cover positive core→engine-host→core sequencing and negative out-of-order/mismatched/wrong-reducer feedback. |
| r[embeddable-agent-engine.cross-reducer-source-rail] | Static rails update the feedback-constructor allowlist so `clankers-engine-host` owns correlated feedback construction, controller composition still owns `SubmitUserPrompt`, and `clankers-agent::turn` only delegates. |
| r[embeddable-agent-engine.host-feedback-construction-seam] | FCIS/source rails allow correlated feedback construction in `clankers-engine-host`, keep `SubmitUserPrompt` construction in controller composition, and reject feedback construction returning to `clankers-agent::turn`. |
| r[embeddable-agent-engine.cancellation-phase-ownership] | Controller/core tests prove pre-engine cancellation reports lifecycle failure with no `EngineInput`; host-runner tests prove post-acceptance cancellation becomes `EngineInput::CancelTurn`; source rails reject direct cancel construction in `clankers-agent::turn`. |
| r[embeddable-agent-engine.no-duplicated-runner-policy] | FCIS/source rails reject runner loops, retry/backoff constants, terminalization helper names, continuation-budget decisions, tool-continuation decisions, duplicated cancellation loops, and shell-local cancellation terminalization in `clankers-agent::turn`. |
| r[embeddable-agent-engine.host-crate-boundary-rails] | Cargo-tree and source rails use concrete forbidden crate/path inventories for `clankers-engine-host` and `clankers-tool-host`. |
| r[embeddable-agent-engine.host-artifact-freshness] | Covered by `host-artifact-refresh`; generated workspace artifacts must include the extracted host crates. |
| r[embeddable-agent-engine.host-artifact-refresh] | Implementation validation must prove workspace manifests, `Cargo.lock`, `flake.nix` test/check crate lists, `build-plan.json`, and generated docs contain the new host crates after running `unit2nix --workspace --force --no-check -o build-plan.json` and `cargo xtask docs`. |
| r[embeddable-agent-engine.host-adapter-parity] | Runtime parity tests cover streaming deltas, tool-call events, tool failures, retry backoff behavior, cancellation behavior, usage updates, hook dispatch, model switching, event ordering, sequential tool scheduling, cancellation-before-next-tool behavior, zero-budget rejection, budget exhaustion, token-limit terminalization, and terminal behavior. |

## Static Rail Design

Extend `crates/clankers-controller/tests/fcis_shell_boundaries.rs` and `scripts/check-llm-contract-boundary.sh`.

Concrete forbidden normal dependencies are enforced by exact `cargo tree`/`cargo metadata` package-name denylist. The denylist is intentionally finite; new crates in the banned categories must be added before they can be relied on by host crates. Source rails below also catch category leakage through public API tokens such as network/runtime handles, timestamps, and provider-shaped request/response types.

- `clankers-engine-host`: `clankers-agent`, `clankers-core`, `clankers-controller`, `clankers-provider`, `clanker-router`, `clankers-db`, `clankers-hooks`, `clankers-plugin`, `clankers-protocol`, `clanker-tui-types`, `clankers-tui`, `ratatui`, `crossterm`, `portable-pty`, `iroh`, `redb`, `reqwest`, `hyper`, `h2`, `tower`, `axum`, `tokio`, `async-std`, `smol`, `actix-rt`, `reqwest-eventsource`, `eventsource-stream`, `chrono`, `time`, `uuid`, `ulid`, `clankers-config`, `clankers-model-selection`.
- `clankers-tool-host`: `clankers-agent`, `clankers-core`, `clankers-controller`, `clankers-provider`, `clanker-router`, `clankers-db`, `clankers-hooks`, `clankers-plugin`, `clankers-protocol`, `clanker-tui-types`, `clankers-tui`, `ratatui`, `crossterm`, `portable-pty`, `iroh`, `redb`, `reqwest`, `hyper`, `h2`, `tower`, `axum`, `tokio`, `async-std`, `smol`, `actix-rt`, `reqwest-eventsource`, `eventsource-stream`, `chrono`, `time`, `uuid`, `ulid`, `clankers-config`, `clankers-model-selection`.

After extraction, `clankers-engine-host::runner` and `clankers-engine-host::runtime` become the allowed seams for correlated `ModelCompleted`, `ModelFailed`, `ToolCompleted`, `ToolFailed`, `RetryReady`, and `CancelTurn` construction; `core_engine_composition.rs` remains the only allowed `SubmitUserPrompt` construction seam, and `clankers-agent::turn` must drop direct feedback construction.

Concrete FCIS inventories:

- `crates/clankers-agent/src/lib.rs` may keep named adapter constants that only pass existing normal/orchestration budgets into engine/host configuration; it must reject retry-backoff constants, retry-delay arithmetic, terminalization helper names, direct engine feedback constructors, and any `EngineEffect::ScheduleRetry` interpretation.
- All non-test `crates/clankers-agent/src/turn/**/*.rs` modules after migration must reject `while let Some(effect)` over engine effects, direct `for effect in &outcome.effects` feedback-driving loops, `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, `EngineInput::CancelTurn`, `ScheduleRetry`, `RetryReady`, `pending_tool_calls`, `pending_model_request`, `retry_attempt`, `retry_budget`, `model_request_slot_budget`, terminal-policy branching on `StopReason::ToolUse` or `StopReason::MaxTokens`, `terminal_failure`, `terminal_failure_outcome`, `terminal_state_with_messages`, `cancel_active_engine_turn`, and shell-local cancellation terminalization strings outside test-only code.
- `crates/clankers-engine-host/src/runner.rs` and `crates/clankers-engine-host/src/runtime/**` may contain `EngineInput::ModelCompleted`, `EngineInput::ModelFailed`, `EngineInput::ToolCompleted`, `EngineInput::ToolFailed`, `EngineInput::RetryReady`, and `EngineInput::CancelTurn`; other `crates/clankers-engine-host/src/**` modules, including `stream`, must reject those constructors. All host modules must reject `EngineInput::SubmitUserPrompt`, retry/backoff constants, `terminal_failure_outcome`, `terminal_state_with_messages`, direct `EngineEvent::TurnFinished` matching, `StopReason::ToolUse`, `StopReason::MaxTokens`, direct continuation-budget mutation, daemon/TUI/DB/plugin supervision types, provider/router discovery types, provider-shaped `CompletionRequest`, shell-native `AgentMessage`, `MessageId`, provider-shaped response tokens (`CompletionResponse`, `ProviderResponse`, `StreamEvent`, `ContentDelta` outside `clanker-message`), network/runtime handles (`reqwest::Client`, `hyper::`, `tokio::runtime::Handle`, `tokio::task::JoinHandle`), `uuid::`, `ulid::`, timestamp construction (`chrono::Utc`, `chrono::DateTime`, `time::OffsetDateTime`), and shell-generated request IDs. Model request translation from `EngineModelRequest` to provider `CompletionRequest` remains in `crates/clankers-agent/src/turn/execution.rs` or a Clankers adapter module, not in the host crate.
- `crates/clankers-tool-host/src/**` must reject `EngineState`, `EngineInput`, `EngineEffect`, `EngineOutcome`, `reduce`, `EngineModelRequest`, `EngineModelResponse`, `EngineToolRequest`, `EngineToolResult`, `RetryReady`, `ScheduleRetry`, `CancelTurn`, terminalization helper names, model/continuation-budget fields, and external truncation helpers such as `clanker_loop::truncate_tool_output`, `clanker_loop::OutputTruncationConfig`, `clankers_util::truncation`, `truncate_head`, and `truncate_tail`, while allowing `EngineToolCall`, `EngineCorrelationId`, and message `Content`.
- `crates/clankers-engine-host/src/**` and `crates/clankers-tool-host/src/**` source/path inventories must reject imports, paths, or string matches for root `src/tools/**` built-in tool modules and `crates/clankers-agent/src/system_prompt.rs`, using anchored path-segment matching only. Forbidden examples include `crate::tools::read`, `crate::tools::write`, `crate::tools::diff`, `crate::tools::web`, `crate::tools::nix`, `clankers::tools::*`, filesystem paths beginning `src/tools/`, and `clankers_agent::system_prompt` / `build_system_prompt`. Bare words such as `read`, `write`, `diff`, `web`, or `nix` are not failures unless they appear as `tools::<name>`, `/tools/<name>`, `src/tools/<name>`, or an import/use path segment rooted at the built-in tool module.
- FCIS failure messages must print the matched file and symbol/token/path.

`clankers-engine-host` is an allowed adapter seam for terminal observation, but it must observe terminal completion through engine-provided helpers such as `EngineEffect::turn_finished_stop_reason()` or equivalent typed accessors, not by directly matching `EngineEvent::TurnFinished` variants. The same helper-based rule applies to `clankers-agent` after migration.

`clankers-tool-host` source rails permit `EngineToolCall`, `EngineCorrelationId`, `Content`, and plain tool request/result content types. They forbid `EngineState`, `EngineInput`, `EngineEffect`, `EngineOutcome`, `reduce`, `EngineModelRequest`, `EngineModelResponse`, `RetryReady`, `ScheduleRetry`, `CancelTurn`, terminalization helper names, and model/continuation-budget fields.

The raw parser/adapter seam test lives in `clankers-agent` or provider-adapter tests, where `clankers-provider`/router dev dependencies are already valid. It feeds provider-native stream bytes/events through the real Clankers normalization entrypoint into provider-neutral `HostStreamEvent` values and then into `clankers-engine-host::stream`; `clankers-engine-host` itself keeps only dependency-light accumulator fixtures.

Workspace artifact refresh tasks must update `Cargo.toml`, `Cargo.lock`, `flake.nix` test/check crate lists, `build-plan.json` via `unit2nix --workspace --force --no-check -o build-plan.json`, and generated docs via `cargo xtask docs` when new crates are introduced.

## Risks / Trade-offs

**Trait explosion** → Mitigate by grouping seams by runtime responsibility and keeping the first runner generic enough for current Clankers plus tests only.

**Behavior drift in event ordering** → Mitigate with focused runtime parity tests over existing `AgentEvent` sequences.

**Tool plugin deadlocks or cancellation regressions** → Mitigate by keeping plugin supervision in existing plugin runtimes and adapting only the call surface.

**Too-large first step** → Mitigate by landing stream accumulator and host traits first, then tool-host extraction, then `clankers-agent` adapter migration.
