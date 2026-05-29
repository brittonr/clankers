# Design: Steel Tool, Plugin, and Subagent Substrate

## Summary

Steel becomes the orchestration substrate for tool, plugin, and subagent calls by moving dispatch selection into a Rust-owned Steel adapter while keeping Rust as the executor and authority boundary. The agent turn loop asks the substrate to plan an invocation, receives a typed action envelope, authorizes the envelope with existing Rust policies, and then executes the selected Rust built-in, WASM plugin, stdio plugin, subagent, or delegate through the existing executor surface.

The invariant is unchanged from Steel turn planning: **Steel orchestrates; Rust enforces and executes.**

## Architecture

### Runtime DTO module

Add a `clankers_runtime::steel_tool_substrate` module that owns stable DTOs and receipt helpers, for example:

- `SteelToolSubstrateProfile`
- `SteelToolCatalogSnapshot`
- `SteelToolDescriptor`
- `SteelToolInvocationRequest`
- `SteelToolInvocationPlan`
- `SteelToolInvocationReceipt`
- `SteelToolExecutorKind::{RustBuiltin,WasmPlugin,StdioPlugin,Subagent}`
- `SteelToolSubstrateStatus::{Authorized,FallbackUsed,Blocked,Denied,Failed}`

The DTOs must be provider-, TUI-, daemon-, and plugin-manager-neutral. They may contain tool name, source label, executor kind, schema hash, input hash, input byte count, capability/resource strings, disabled status, redaction class, call id, child/subagent status metadata, and receipt destination. They must not contain raw prompts, subagent task bodies/transcripts, raw provider payloads, raw plugin bodies, credentials, raw stdout/stderr streams, or unbounded tool output.

### Steel host-function surface

The reviewed host-function surface is intentionally small:

- `steel.host.tool.list` returns a bounded redacted catalog snapshot.
- `steel.host.tool.call` returns one typed invocation plan for one requested call.

Steel scripts do not execute tools. Host functions return data that Rust validates. Future host functions such as cancellation or batch calls require separate Cairn requirements and policy profiles.

### Agent turn dispatch seam

Add a Rust-owned dispatch port in the current tool execution path before direct `Tool::execute` / `PluginTool::execute` / `SubagentTool` / `DelegateTool` dispatch. The port receives the engine tool call, call id, current tool catalog, disabled-tool state, cancellation state, and capability context. In comparison mode it records the Steel plan but executes the current direct path. In default mode it executes only after the Steel plan authorizes the same call through Rust policy.

The existing direct path remains the fallback oracle until default rollout evidence proves parity.

### Rust built-in executor adapter

Built-in tools continue to implement the existing `Tool` trait during migration. The substrate adapter wraps an `Arc<dyn Tool>` as an executor backend and calls `Tool::execute` only after:

1. Steel returns a typed plan for `SteelToolExecutorKind::RustBuiltin`.
2. Rust verifies tool name, call id, input hash, executor kind, schema version, disabled-tool state, and capability requirements.
3. The existing cancellation, hook pipeline, progress, database/search service, accumulator, and output truncation behavior is installed.

Direct built-in execution outside this adapter is allowed only in disabled/comparison/fallback mode and must be covered by a boundary rail.

### WASM/Extism plugin executor adapter

WASM plugin calls use the same invocation envelope with executor kind `WasmPlugin`, plugin name, tool name, and reviewed exported function name. The adapter should prefer `PluginHostFacade` or a narrow wrapper over `PluginManager` so active-plugin filtering, plugin summaries, event queues, and future mixed-runtime behavior stay centralized.

Rust still owns plugin manifest validation, WASM loading, host-call processing, panic isolation, permissions, and conversion back to `ToolResult`. Steel sees only redacted catalog metadata and the request hash.

### Stdio plugin executor adapter

Stdio plugin calls use executor kind `StdioPlugin` with the existing stdio tool-call lifecycle. The adapter must preserve:

- `start_stdio_tool_call` / result-event channel semantics
- cancellation via `cancel_stdio_tool_call` and `abandon_stdio_tool_call`
- timeout and cancel-grace behavior
- progress/UI event drainage
- supervisor run-id and restart/disable race protections
- restricted launch policy, Landlock/seccomp failure-closed behavior, and plugin state-dir rules

Steel must not own async polling or process handles. Rust receives the typed plan, starts the stdio call, drains events, and produces the final receipt.

### Subagent and delegate executor adapter

Subagent and delegate calls use executor kind `Subagent` and continue through the existing `SubagentTool` / `DelegateTool` implementations after substrate authorization. Rust owns child-agent execution details: in-process daemon actors via `ActorContext`, subprocess fallback, remote prompt RPC where configured, process monitoring, watchdogs, subagent panel events, pane limits, kill/cancel requests, worker metadata, and session/controller construction.

Steel may choose or authorize the invocation route, but it must not spawn actors, construct `SessionController`, access `ProcessRegistry`, allocate panes, kill PIDs, open sockets, or receive raw child prompts/transcripts. The plan/receipt carries hashes and bounded labels for the requested subagent/delegate call; Rust passes the original tool parameters to the executor only after policy and capability checks succeed.

## Policy and authorization

The substrate profile names allowed host actions, executor kinds, runtime budgets, fallback mode, redaction policy, and receipt destination prefix. Rust checks the plan against:

- profile allowed host action and executor kind
- disabled-tool policy and user tool filters
- UCAN/session capability/resource authority
- plugin active/loaded state and manifest launch policy
- subagent/delegate concurrency, actor/subprocess mode, process-monitor, watchdog, and panel-routing policy
- input hash and input byte budget
- output truncation/redaction policy
- cancellation state before and during execution

A denied or malformed Steel plan must not execute through a direct tool/plugin/subagent fallback unless the profile explicitly says comparison/fallback mode is active. Fallback receipts must say that Rust-native direct dispatch was used.

## Receipt policy

Each invocation produces a bounded receipt with schema, call id, tool name, source label, executor kind, profile, script/policy hash, request hash, input hash, output hash when available, authorization status, fallback status, redaction class, child/subagent status when applicable, and safe error class. Receipts must omit raw args when classified sensitive, raw prompts, raw subagent prompts/transcripts, credentials, provider payloads, raw plugin stdout/stderr, raw WASM output bodies, raw script source, and uncontrolled absolute paths.

## Rollout modes

- `disabled`: use current direct dispatch and emit no Steel-authorship claim.
- `comparison`: evaluate Steel substrate and receipts, execute direct dispatch as oracle.
- `default`: execute only authorized Steel-mediated plans, with Rust fallback only when policy allows.
- `block`: fail closed on Steel substrate failure or denial.

A settings shape such as `steelToolSubstrate.{enabled,rolloutStage,fallbackMode,profilePath,scriptPath,disabledExecutors}` should mirror the existing `steelTurnPlanning` kill-switch and artifact validation behavior.

## Verification plan

- Source-boundary rail proving CLI/daemon/TUI/provider/plugin-manager code cannot import Steel interpreter internals or bypass the substrate adapter in default mode.
- DTO/serde fixture tests for catalog snapshots, invocation plans, receipts, redaction, malformed schema, executor-kind mismatch, input-hash mismatch, disabled-tool denial, and fallback/block behavior.
- Built-in tool fixture proving one read-only and one mutating/progress-emitting built-in preserve hooks, cancellation, accumulator/truncation, and capability denial behavior.
- WASM plugin fixture using the existing test plugin to prove envelope wrapping, manifest/active-plugin checks, host-call permission behavior, panic/error conversion, and receipt redaction.
- Stdio plugin fixture proving progress, timeout, cancellation, disconnect, disabled/restarted supervisor, and restricted sandbox failure remain Rust-owned.
- Runtime dogfood rail proving a real agent turn can call representative built-in, WASM, stdio, and subagent/delegate executors through the Steel substrate with deterministic receipts.

## Non-goals and guardrails

The substrate is not an OS sandbox, plugin runtime, or agent process manager. Steel must not load WASM, spawn stdio processes, spawn/kill subagents, call providers, mutate daemon/session state, or access the filesystem/network directly. Every effect remains behind Rust executor adapters and existing policy gates.
