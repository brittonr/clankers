<!-- This file is checked by `scripts/check-embedded-sdk-api.rs`. Keep support labels intentional. -->

<div class="generated-warning">
⚡ Checked embedded SDK API inventory. Run <code>scripts/check-embedded-sdk-api.rs</code> to verify source mappings and support labels.
</div>

# Embedded SDK API Inventory

This inventory defines the documented embedded-agent SDK surface for `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, and the shared message contracts that those crates use.

Support labels:

- `supported` — stable embedding entrypoint for the current Clankers crate version line. Removing, renaming, or repurposing it requires an explicit migration note.
- `optional-support` — supported when a host intentionally opts into the companion concern, such as prompt lifecycle or provider-neutral streaming contracts.
- `experimental` — public but not yet promised as stable embedding API.
- `unsupported-internal` — public because of current crate layout or tests, but not an advertised stable embedded SDK entrypoint.

## Inventory

| Entry | Crate | Kind | Stability | Source |
|---|---|---|---|---|
| `ENGINE_BUDGET_EXHAUSTED_NOTICE` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_CONTRACT_VERSION` | `clankers-engine` | constant | supported | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_CORRELATION_SEQUENCE_STEP` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_DEFAULT_RETRY_DELAY_COUNT` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_DEFAULT_RETRY_DELAYS` | `clankers-engine` | constant | supported | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_FIRST_RETRY_DELAY_SECONDS` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_INITIAL_MODEL_REQUEST_SEQUENCE` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_MIN_MODEL_REQUEST_SLOT_BUDGET` | `clankers-engine` | constant | supported | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_MODEL_REQUEST_PREFIX` | `clankers-engine` | constant | supported | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_MODEL_REQUEST_SLOT_COST` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_SECOND_RETRY_DELAY_SECONDS` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `ENGINE_SUBMIT_PROMPT_NOTICE` | `clankers-engine` | constant | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `EngineBufferedToolResult` | `clankers-engine` | struct | experimental | `crates/clankers-engine/src/lib.rs` |
| `EngineCorrelationId` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineEffect` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineEvent` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineInput` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineMessage` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineMessageRole` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineModelRequest` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineModelResponse` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineOutcome` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EnginePromptSubmission` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineRejection` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineRequestTemplate` | `clankers-engine` | struct | unsupported-internal | `crates/clankers-engine/src/lib.rs` |
| `EngineRetryPolicy` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineState` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineTerminalFailure` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineToolCall` | `clankers-engine` | struct | supported | `crates/clankers-engine/src/lib.rs` |
| `EngineTurnPhase` | `clankers-engine` | enum | supported | `crates/clankers-engine/src/lib.rs` |
| `reduce` | `clankers-engine` | function | supported | `crates/clankers-engine/src/lib.rs` |
| `CAPABILITY_DENIED_ERROR_PREFIX` | `clankers-engine-host` | constant | unsupported-internal | `crates/clankers-engine-host/src/lib.rs` |
| `HOST_CANCELLED_REASON` | `clankers-engine-host` | constant | supported | `crates/clankers-engine-host/src/lib.rs` |
| `MISSING_TOOL_ERROR_PREFIX` | `clankers-engine-host` | constant | unsupported-internal | `crates/clankers-engine-host/src/lib.rs` |
| `TOOL_CANCELLED_ERROR_PREFIX` | `clankers-engine-host` | constant | unsupported-internal | `crates/clankers-engine-host/src/lib.rs` |
| `AdapterDiagnostic` | `clankers-engine-host` | struct | supported | `crates/clankers-engine-host/src/lib.rs` |
| `CancellationSource` | `clankers-engine-host` | trait | supported | `crates/clankers-engine-host/src/lib.rs` |
| `EngineEventSink` | `clankers-engine-host` | trait | supported | `crates/clankers-engine-host/src/lib.rs` |
| `EngineRunReport` | `clankers-engine-host` | struct | supported | `crates/clankers-engine-host/src/lib.rs` |
| `EngineRunSeed` | `clankers-engine-host` | struct | supported | `crates/clankers-engine-host/src/lib.rs` |
| `HostAdapterComponent` | `clankers-engine-host` | enum | supported | `crates/clankers-engine-host/src/lib.rs` |
| `HostAdapterError` | `clankers-engine-host` | enum | supported | `crates/clankers-engine-host/src/lib.rs` |
| `HostAdapters` | `clankers-engine-host` | struct | supported | `crates/clankers-engine-host/src/lib.rs` |
| `ModelHost` | `clankers-engine-host` | trait | supported | `crates/clankers-engine-host/src/lib.rs` |
| `ModelHostOutcome` | `clankers-engine-host` | enum | supported | `crates/clankers-engine-host/src/lib.rs` |
| `RetrySleeper` | `clankers-engine-host` | trait | supported | `crates/clankers-engine-host/src/lib.rs` |
| `UsageObservation` | `clankers-engine-host` | struct | supported | `crates/clankers-engine-host/src/lib.rs` |
| `UsageObservationKind` | `clankers-engine-host` | enum | supported | `crates/clankers-engine-host/src/lib.rs` |
| `UsageObserver` | `clankers-engine-host` | trait | supported | `crates/clankers-engine-host/src/lib.rs` |
| `run_engine_turn` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/lib.rs` |
| `runtime` | `clankers-engine-host` | module | supported | `crates/clankers-engine-host/src/lib.rs` |
| `stream` | `clankers-engine-host` | module | optional-support | `crates/clankers-engine-host/src/lib.rs` |
| `DEFAULT_TOOL_FAILURE_MESSAGE` | `clankers-engine-host` | constant | unsupported-internal | `crates/clankers-engine-host/src/runtime.rs` |
| `cancel_turn_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `model_completed_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `model_failed_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `retry_ready_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `tool_completed_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `tool_failed_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `tool_feedback_input` | `clankers-engine-host` | function | supported | `crates/clankers-engine-host/src/runtime.rs` |
| `EMPTY_TOOL_INPUT_JSON` | `clankers-engine-host` | constant | unsupported-internal | `crates/clankers-engine-host/src/stream.rs` |
| `HostStreamEvent` | `clankers-engine-host` | enum | optional-support | `crates/clankers-engine-host/src/stream.rs` |
| `ProviderStreamError` | `clankers-engine-host` | struct | optional-support | `crates/clankers-engine-host/src/stream.rs` |
| `StreamAccumulator` | `clankers-engine-host` | struct | optional-support | `crates/clankers-engine-host/src/stream.rs` |
| `StreamAccumulatorError` | `clankers-engine-host` | enum | optional-support | `crates/clankers-engine-host/src/stream.rs` |
| `StreamFoldResult` | `clankers-engine-host` | struct | optional-support | `crates/clankers-engine-host/src/stream.rs` |
| `DEFAULT_TOOL_MAX_BYTES` | `clankers-tool-host` | constant | supported | `crates/clankers-tool-host/src/lib.rs` |
| `DEFAULT_TOOL_MAX_LINES` | `clankers-tool-host` | constant | supported | `crates/clankers-tool-host/src/lib.rs` |
| `CapabilityChecker` | `clankers-tool-host` | trait | supported | `crates/clankers-tool-host/src/lib.rs` |
| `CapabilityDecision` | `clankers-tool-host` | enum | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolCatalog` | `clankers-tool-host` | trait | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolDescriptor` | `clankers-tool-host` | struct | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolExecutor` | `clankers-tool-host` | trait | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolHook` | `clankers-tool-host` | trait | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolHostError` | `clankers-tool-host` | enum | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolHostOutcome` | `clankers-tool-host` | enum | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolOutputAccumulator` | `clankers-tool-host` | struct | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolTruncationLimits` | `clankers-tool-host` | struct | supported | `crates/clankers-tool-host/src/lib.rs` |
| `ToolTruncationMetadata` | `clankers-tool-host` | struct | supported | `crates/clankers-tool-host/src/lib.rs` |
| `tool_call_id` | `clankers-tool-host` | function | supported | `crates/clankers-tool-host/src/lib.rs` |
| `contracts` | `clanker-message` | module | optional-support | `crates/clanker-message/src/lib.rs` |
| `message` | `clanker-message` | module | optional-support | `crates/clanker-message/src/lib.rs` |
| `result_streaming` | `clanker-message` | module | optional-support | `crates/clanker-message/src/lib.rs` |
| `streaming` | `clanker-message` | module | optional-support | `crates/clanker-message/src/lib.rs` |
| `tool_result` | `clanker-message` | module | optional-support | `crates/clanker-message/src/lib.rs` |
| `ThinkingConfig` | `clanker-message` | struct | supported | `crates/clanker-message/src/contracts.rs` |
| `ToolDefinition` | `clanker-message` | struct | supported | `crates/clanker-message/src/contracts.rs` |
| `Usage` | `clanker-message` | struct | supported | `crates/clanker-message/src/contracts.rs` |
| `AgentMessage` | `clanker-message` | enum | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `AssistantMessage` | `clanker-message` | struct | experimental | `crates/clanker-message/src/message.rs` |
| `BashExecutionMessage` | `clanker-message` | struct | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `BranchSummaryMessage` | `clanker-message` | struct | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `CompactionSummaryMessage` | `clanker-message` | struct | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `Content` | `clanker-message` | enum | supported | `crates/clanker-message/src/message.rs` |
| `CustomMessage` | `clanker-message` | struct | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `ImageSource` | `clanker-message` | enum | supported | `crates/clanker-message/src/message.rs` |
| `MessageId` | `clanker-message` | struct | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `StopReason` | `clanker-message` | enum | supported | `crates/clanker-message/src/message.rs` |
| `ToolResultMessage` | `clanker-message` | struct | experimental | `crates/clanker-message/src/message.rs` |
| `UserMessage` | `clanker-message` | struct | experimental | `crates/clanker-message/src/message.rs` |
| `generate_id` | `clanker-message` | function | unsupported-internal | `crates/clanker-message/src/message.rs` |
| `ResultChunk` | `clanker-message` | struct | optional-support | `crates/clanker-message/src/result_streaming.rs` |
| `ToolResultAccumulator` | `clanker-message` | struct | optional-support | `crates/clanker-message/src/result_streaming.rs` |
| `TruncationConfig` | `clanker-message` | struct | optional-support | `crates/clanker-message/src/result_streaming.rs` |
| `ContentDelta` | `clanker-message` | enum | optional-support | `crates/clanker-message/src/streaming.rs` |
| `MessageMetadata` | `clanker-message` | struct | optional-support | `crates/clanker-message/src/streaming.rs` |
| `StreamDelta` | `clanker-message` | type | optional-support | `crates/clanker-message/src/streaming.rs` |
| `StreamEvent` | `clanker-message` | enum | optional-support | `crates/clanker-message/src/streaming.rs` |
| `ToolResult` | `clanker-message` | struct | optional-support | `crates/clanker-message/src/tool_result.rs` |
| `ToolResultContent` | `clanker-message` | enum | optional-support | `crates/clanker-message/src/tool_result.rs` |
