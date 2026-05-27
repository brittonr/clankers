# Embedded Agent SDK

Clankers can be embedded as a Rust engine without importing the terminal app, daemon, provider discovery, session database, TUI, prompt assembly, plugin supervision, or built-in tool bundles. This guide defines supported crates, adapter contracts, validation policy, and recipe coverage for the productized embedded-agent path.

## Supported crate set

The supported embedding surface is the small reducer/host layer:

| Crate | Role | Supported entrypoints |
|---|---|---|
| `clankers-engine` | Pure turn-state reducer for accepted model/tool work | `EngineState`, `EngineInput`, `EnginePromptSubmission`, `EngineOutcome`, `EngineEffect`, `EngineModelRequest`, `EngineModelResponse`, `EngineTerminalFailure`, `EngineToolCall`, `reduce` |
| `clankers-engine-host` | Async effect interpreter that drives the reducer through caller adapters | `EngineRunSeed`, `HostAdapters`, `run_engine_turn`, `ModelHost`, `ModelHostOutcome`, `RetrySleeper`, `EngineEventSink`, `CancellationSource`, `UsageObserver`, stream accumulator types, runtime input helpers |
| `clankers-tool-host` | Provider-neutral tool execution contracts and output truncation | `ToolExecutor`, `ToolHostOutcome`, `ToolOutputAccumulator`, `ToolTruncationLimits`, `ToolTruncationMetadata`, `ToolCatalog`, `CapabilityChecker`, `ToolHook` |
| `clankers-adapters` | Shell-free reusable adapter bricks, embedded tool catalog DTOs, and capability-pack presets | `MemoryEventSink`, `AtomicCancellationSource`, `NoopRetrySleeper`, `CollectingUsageObserver`, `ScriptedModelHost`, `ScriptedToolExecutor`, `EmbeddedToolCatalog`, `CapabilityPack` |
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
- MCP, ACP, and daemon attach protocols as generic SDK dependencies.

Daemon, MCP, and ACP integrations remain supported as **application-edge** surfaces when a product wants a process boundary or existing Clankers shell behavior. They are not imported by the reusable embedded SDK crates or recipes.
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

The checked-in consumer fixture under `examples/embedded-agent-sdk/` is the executable form of this sketch. `examples/embedded-minimal-kit/` uses the reusable adapter bricks for the smallest product kit, `examples/embedded-tool-kit/` covers successful tool execution plus missing-tool, tool-error, capability-denial, and truncation paths, `examples/embedded-provider-adapter/` shows a product-owned `ModelHost` converting `EngineModelRequest` into local provider IO without importing `clankers-provider`, `examples/embedded-session-store/` shows host-owned session persistence with product DTOs, an in-memory product store, restored-history model-request assertions, and missing-session fail-closed behavior without importing Clankers storage/session shells, and `examples/embedded-product-workbench/` composes those seams together in one product-style dogfood recipe with provider, tool-catalog, session-store, receipt, restored-context, and fail-closed assertions. These examples must stay outside the workspace crate graph and depend only on SDK crates plus application-owned executor/test helpers.

The provider-adapter kit is fixture backed. `examples/embedded-provider-adapter/fixtures/provider-adapter-fixtures.json` pins explicit request, completed-response, retryable-failure, terminal-failure, usage-accounting, and `ProductModelProfile` data. `scripts/check-provider-adapter-kit.rs` verifies those fixtures, the product-owned `ProductProviderAdapter` implementation, and the dependency boundary so expected provider shapes are not derived from the adapter under test and the generic SDK path stays free of `clankers-provider`, router daemon RPC, OAuth stores, provider discovery, and live network credentials.

The session-resume-brick evidence is fixture backed across two product-shaped stores. `examples/embedded-session-store/session-resume-evidence.json` pins the `embedded-session-store` and `embedded-product-workbench` DTO/store shapes, expected restored `EngineModelRequest` role/text order, missing-session errors, and forbidden shell dependencies. `scripts/check-session-resume-brick.rs` verifies that both examples keep product-owned session/message DTOs, preserve restored user/tool/assistant context into the follow-up model request, fail closed for missing sessions before model/tool execution, and avoid `clankers-session`, `clankers-db`, JSONL restore shells, daemon sockets, and TUI/session restore logic. This is convergence evidence only: reusable session APIs require a later Cairn instead of importing storage ownership into green SDK crates.

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

## Composition kits, catalogs, and capability packs

`clankers-adapters` provides boring reusable bricks for the common seams: in-memory event capture, atomic cancellation, no-op retry sleeping, usage collection, scripted/fake model responses, scripted tool execution, typed embedded tool catalogs, catalog-backed tool execution, and capability-pack presets. Each brick is replaceable by an app-owned implementation of the same host/tool trait; products should use these as defaults or tests, not as a reason to couple application policy back into SDK crates.

Declarative tool catalogs are parser-neutral DTOs. JSON is supported by the current serde model, but public semantics are the Rust data model: tool name, description, runtime kind, capabilities, approval policy, redaction policy, and input schema. Validation is fail-closed for duplicate names, missing descriptions, unknown runtime kinds, unsafe capabilities without explicit per-call approval, and secret-adjacent tools without redaction. Mutating, shell, network, raw-log, and secret-adjacent capabilities are explicit opt-ins.

Capability packs are named snapshots, not open-ended role expansion. Product-facing presets `embedding_safe`, `read_only`, `networkless_coding`, `project_local_edit`, and `human_approved_shell` preserve exact capability sets under tests so later additions are intentional and reviewed. `embedding_safe`, `read_only`, and `networkless_coding` exclude mutating, shell, network, raw-log, and secret-adjacent capabilities; `human_approved_shell` is an explicit opt-in danger boundary. Legacy `tool_user` and `operator` helpers remain compatibility aliases for existing consumers.

## Adapter-only modular coupling rules

Keep generic crates dependency-inverted:

1. `clankers-engine` owns accepted turn policy only. It may use plain shared message/content data, but it must not import Clankers shell/runtime types.
2. `clankers-engine-host` owns effect interpretation and correlation plumbing only. It calls host traits; it must not discover providers, tools, prompts, sessions, daemons, plugins, network services, or runtime handles.
3. `clankers-tool-host` owns reusable tool outcome and truncation contracts only. It must not supervise plugins or call built-in tools.
4. `clanker-message` stays provider/router-neutral. Provider-native request shaping belongs in host adapters.
5. Application edge code may compose SDK crates with Clankers runtime crates, but that code is not part of the generic SDK surface.

`scripts/check-embedded-agent-sdk.rs` is the Rust-owned required acceptance rail for these rules. It composes API inventory, docs freshness, example execution, feature/default checks, dependency denylist checks, source boundary checks, embedded lego contract validation, release-receipt generation, and focused Clankers parity rails. `scripts/check-embedded-agent-sdk.sh` remains only a compatibility wrapper that delegates to the Rust rail.

## Lego contract policy and evidence

The product-facing lego policy lives under `policy/embedded-lego/`. `lego-contracts.ncl` is the author-time Nickel contract sketch for typed, mergeable policy boundaries; `lego-contracts.json` is the checked export consumed by repository validation. Generic SDK crates do not evaluate Nickel at runtime.

`scripts/check-embedded-lego-contracts.rs` validates the exported contract across the current lego backlog surfaces: green/yellow/red crate boundaries, capability-pack composition, declarative tool catalog manifest rules, real-product dogfood evidence, provider-adapter fixtures, session/resume evidence, and plugin/tool runtime dispatch separation. The checker emits `target/embedded-sdk-release/lego-contracts-receipt.json` with BLAKE3 hashes for the policy, contract sketch, product-workbench/session/tool/provider examples, the session-resume fixture, the tool-catalog manifest fixture, and generated API/docs evidence.

`examples/embedded-tool-kit/tool-catalog-manifest.json` is the checked tool-catalog-manifest fixture. `scripts/check-tool-catalog-manifest.rs` validates that catalog loading/export is runtime-neutral: it normalizes `EmbeddedToolCatalog`-compatible metadata without starting stdio, loading Extism, calling the network, reading secrets, or executing product tools. It also pins fail-closed denial fixtures for duplicate names, invalid schemas, unknown runtime kinds, unsafe capability defaults, missing redaction, and undeclared dangerous capabilities, plus a bounded truncation fixture. The checker emits `target/embedded-sdk-release/tool-catalog-manifest-receipt.json` with BLAKE3 hashes for the authored manifest, normalized metadata, denial fixtures, truncation fixture, docs, policy, and canonical spec evidence.

`policy/embedded-lego/capability-pack-composition.json` is the checked capability-pack composition fixture. `scripts/check-capability-pack-composition.rs` validates deterministic merge order (`observe`, `respond`, `read_project`, `edit_project`, `mutate_project`, `shell`, `network`, `raw-log`, `secret-adjacent`), exact `safe-only` safe-pack snapshots, order-independent overlays, and fail-closed diagnostics for safe-pack plus shell/network/secret-adjacent expansion unless a product-owned approval policy explicitly allows the dangerous pack. The checker emits `target/embedded-sdk-release/capability-pack-composition-receipt.json` with BLAKE3 hashes for fixture, policy, docs, spec, and normalized composition snapshots.

`policy/embedded-lego/plugin-runtime-dispatch.json` is the checked plugin/tool runtime dispatch fixture. `scripts/check-plugin-runtime-dispatch.rs` validates that Extism, stdio, built-in, and product-owned runtime entries have one dispatch owner, launch/sandbox/capability/redaction policy is checked before dispatch, and non-Extism entries never flow through eager WASM loading or another runtime loader. The checker emits a plugin-runtime-dispatch receipt at `target/embedded-sdk-release/plugin-runtime-dispatch-receipt.json` with BLAKE3 hashes for the fixture, policy, docs, spec, and plugin source guards. This rail is app-edge/yellow evidence; it does not promote plugin supervision or built-in tool bundles into green SDK crates.

`scripts/check-session-resume-brick.rs` additionally emits `target/embedded-sdk-release/session-resume-brick-receipt.json` for the two-product restored-context convergence matrix. These hashes are deterministic drift evidence, not authorization; any semantic policy or fixture change should intentionally update the checked policy, docs, tests, and receipts together.

`examples/embedded-product-workbench/dogfood-manifest.json` is the checked real-product dogfood manifest. `scripts/check-real-product-dogfood.rs` validates that the workbench uses only policy-approved green SDK crates, declares product-owned provider/session/tool seams, excludes shell/runtime surfaces, and then emits deterministic dependency-boundary, sanitized-transcript, and BLAKE3 receipt evidence under `target/embedded-sdk-release/product-dogfood/`. The generated transcript is sanitized fixture evidence: it proves the product-style seams and fail-closed paths without live credentials, network access, daemon startup, provider discovery, OAuth stores, or user-local Clankers session state.

Reusable app-owned glue found while extending this dogfood path should become a follow-up Cairn before entering green SDK crates; do not silently widen green dependencies from product evidence alone.

## Feature and default policy

Current SDK crates are intended to work with their default features for the minimal embedding path:

- `clankers-engine`: no optional features; depends on `clanker-message` and `serde_json`.
- `clankers-engine-host`: no optional features; depends on `clankers-engine`, `clankers-tool-host`, `clanker-message`, `serde`, `serde_json`, and `thiserror`.
- `clankers-tool-host`: no optional features; depends on `clankers-engine`, `clanker-message`, `serde`, `serde_json`, and `thiserror`.
- `clankers-adapters`: no optional features; depends only on SDK crates plus `serde`, `serde_json`, and `thiserror` for DTO validation and reusable test/product bricks.
- `clanker-message`: default crate features are acceptable for embedding; it owns shared content/usage/message data, not application shells.
- `clankers-core`: optional for hosts that want prompt lifecycle/follow-up reduction before engine submission; not required by the minimal engine-host example.

## Product embedding crate guidance

- **Green**: `clanker-message`, `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, and `clankers-adapters` are the checked product-embedding crates. `clankers-core` is green only for hosts that want prompt lifecycle reduction before an engine turn.
- **Yellow**: app-edge crates such as daemon, MCP, ACP, runtime extension services, provider adapters, storage, or plugin boundaries may be composed by a product, but only behind a product-owned integration layer and not as transitive dependencies of generic SDK crates. Product-owned session/message DTOs and storage schemas are yellow app-edge concerns unless a later Cairn promotes a reusable storage API after multiple products converge on the same shape.
- **Red**: `clankers-agent`, `clankers-controller`, `clankers-provider`, `clanker-router`, `clankers-db`, `clankers-protocol`, `clankers-tui`, prompt/skill bundles, Matrix, iroh/P2P, ratatui, and crossterm are not generic product SDK dependencies.

The minimal embedding path must not require features that pull in daemon, TUI, provider discovery, database, prompt assembly, plugin supervision, built-in tools, Matrix, iroh, ratatui, or crossterm. Any future optional SDK feature must be documented here and validated by the feature/default-policy checker before it is advertised.

## Support, versioning, and migration policy

Clankers currently versions the SDK crates with the repository crate versions. Supported embedding entrypoints are the ones documented in this guide and classified in `docs/src/generated/embedded-sdk-api.md`.

Product embedders should capture a release receipt after the acceptance rail succeeds:

```bash
scripts/check-embedded-agent-sdk.rs
scripts/emit-embedded-sdk-release-receipt.rs --output target/embedded-sdk-release/receipt.json
```

The receipt records the current commit/status, BLAKE3 hashes for the SDK guide, generated API inventory, canonical embedded composition spec, acceptance scripts, brick inventory stability policy/checker, and standalone embedded examples, plus the green/yellow/red boundary classification. Capture it from a clean committed checkout before claiming product embedding readiness; dirty development runs remain useful because the receipt preserves `git status --short --branch` instead of hiding local changes.

Compatibility expectations:

- Supported entrypoints should not be removed, renamed, or semantically repurposed without an explicit migration note and refreshed `scripts/check-brick-inventory-stability.rs` receipt.
- Compatibility aliases are supported migration shims and must name the canonical replacement before they are removed.
- Additions are allowed when they do not force forbidden shell/runtime dependencies into generic SDK crates.
- Unsupported/internal exported items may change without migration notes and must not be advertised as stable embedding API.
- Application-layer adapters that use `clankers-agent`, provider discovery, daemon, TUI, DB, prompts, or plugins are outside the generic SDK compatibility promise.

Migration notes for SDK changes belong in this guide under this section until a dedicated release-notes file exists. Each migration note should name the affected entrypoint, the replacement or adapter change, and the validation command that proves the new path.

## Validation checklist

Before claiming embedded SDK readiness, run:

```bash
scripts/check-embedded-agent-sdk.rs
```

That bundle must prove:

- documented entrypoints map to exported items or example paths;
- public API inventory is fresh;
- stale docs fail the checker;
- `examples/embedded-agent-sdk/` runs positive and negative adapter paths;
- executable kit examples cover minimal adapter bricks, tool catalogs, product-owned provider adapter conversion, host-owned session persistence/resume, and one combined product-workbench dogfood recipe that composes provider, tool, and session seams together;
- real-product dogfood validation checks `examples/embedded-product-workbench/dogfood-manifest.json` before accepting runtime evidence and emits dependency-boundary, sanitized-transcript, and BLAKE3 receipt artifacts under `target/embedded-sdk-release/product-dogfood/`;
- release-receipt generation records commit/status metadata, verification commands, green/yellow/red boundaries, and BLAKE3 hashes for embedded SDK docs/spec/scripts/examples;
- example dependency graph excludes Clankers shell/runtime crates and UI/network crates listed in the Cairn change;
- feature/default policy matches manifests and a minimal example build;
- generic SDK crates reject provider/router, daemon/TUI, database, networking, timestamp, shell-generated ID, runtime-handle, provider-shaped request/response, hidden-global-service, and concrete Clankers runtime leakage;
- default `clankers-agent::Agent` still routes through the reusable host runner and preserves streaming, tool, retry, cancellation, usage, and terminal behavior.

## Migration notes

No embedded SDK migrations have been published yet. The first compatibility baseline is the API inventory generated by this change.
