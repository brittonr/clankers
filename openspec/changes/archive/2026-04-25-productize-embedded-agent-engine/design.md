## Context

The reusable engine work is implemented and archived: `clankers-core` owns prompt lifecycle policy, `clankers-engine` owns accepted model/tool turn policy, `clankers-engine-host` executes engine effects through caller-supplied adapters, and `clankers-tool-host` owns reusable tool outcomes/truncation. Clankers itself now routes turns through the shared host-runner seam, but external consumers still lack a supported path that answers: which crates are stable, which APIs to call, how to implement adapters, how to build a minimal agent, and which command proves the embedding surface is fresh.

## Verification Summary

The implementation should finish with one durable command: `scripts/check-embedded-agent-sdk.sh`. That bundle should run the minimal external-consumer example, docs/API reference checks, SDK public API inventory checks, feature/default-policy checks, dependency/source boundary rails, generated artifact freshness checks, and focused Clankers agent parity rails. The parity rails must explicitly cover streaming deltas, tool execution and tool failures, retry/backoff behavior, cancellation behavior, usage observations/final summaries, and terminal stop/error behavior while proving the default Clankers assembly still routes through the reusable host runner.

## Goals / Non-Goals

**Goals:**

- Make the embedded engine usable by a Rust project that does not import `clankers-agent` or Clankers app shells.
- Document the supported crate set, public entrypoints, adapter contracts, support/versioning policy, feature/default policy, and acceptance command.
- Add a minimal external-consumer example or fixture that executes a full prompt → model/tool → terminal turn through `clankers-engine-host` with in-memory adapters.
- Keep validation deterministic: public API inventory, dependency denylist, docs/reference freshness, example execution, and Clankers parity should all be checked by `scripts/check-embedded-agent-sdk.sh`.

**Non-Goals:**

- Shipping provider-specific, networked, daemon-backed, Matrix-backed, or TUI-backed embedding adapters as part of the minimal SDK path.
- Moving prompt assembly, provider discovery, session DB ownership, plugin supervision, built-in tool bundles, daemon protocol, or TUI rendering into generic engine crates.
- Removing `clankers-agent::Agent` or changing it from the default Clankers assembly.
- Promising semantic-versioned stability for undocumented internals; stability applies only to the documented SDK surface and its inventory.

## Decisions

### 1. The SDK surface is documentation plus validated inventory, not a new facade crate

**Choice:** Productize the existing crate split first: `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, and `clanker-message` are the supported SDK surface. Add docs and API inventory over that surface instead of introducing a new `clankers-sdk` facade crate in this change.

**Rationale:** The previous extraction work already created the right dependency-light crates. A facade crate would add another API boundary before the current one is proven by an external consumer. The immediate gap is discoverability and validation, not another abstraction layer.

**Alternative:** Create `clankers-sdk` as a curated facade. Rejected for this change because it would hide whether the underlying crates are actually usable by external hosts and could duplicate the public API inventory problem.

### 2. The external-consumer example is a standalone fixture crate

**Choice:** Add a minimal checked-in fixture under `examples/embedded-agent-sdk/` with its own `Cargo.toml`. It should depend only on the SDK crates plus an application-owned executor/helper dependency if needed to drive async futures. It should not be a workspace crate that inherits the full Clankers dependency graph by accident.

**Rationale:** A standalone fixture proves the thing external users need: an independent crate can assemble a turn without `clankers-agent`, daemon, TUI, provider/router, DB, or prompt assembly. Keeping its manifest explicit makes dependency audit deterministic.

**Alternative:** Add only unit tests inside `clankers-engine-host`. Rejected because those tests prove internal behavior, not external-consumer ergonomics or dependency cleanliness.

### 3. Adapter recipes live near docs and point to executable examples

**Choice:** Add an embedded-agent SDK guide in the docs tree and keep adapter recipes concise: model host, tool executor, retry sleeper, event sink, cancellation source, usage observer, and transcript conversion. Each recipe must point to the external fixture or a focused test that covers both success and failure behavior.

**Rationale:** The engine-host traits are intentionally small, but embedders need examples of error classification, cancellation precedence, retry sleeping, usage observation diagnostics, and tool outcome mapping. Recipes that link to tests stay reviewable and less likely to drift.

**Alternative:** Put long doc comments on every trait only. Rejected because trait docs alone do not show how a complete host assembles the pieces.

### 4. Dependency inversion is a hard SDK boundary

**Choice:** Treat adapter-only coupling as an architectural rule, not just documentation style. Generic SDK crates expose plain engine data and host traits/interfaces; concrete providers, tools, prompts, storage, events, cancellation, and runtime choices are composed at the application edge. Boundary rails should fail if the generic SDK crates instantiate or require Clankers provider discovery, daemon/TUI, DB, prompt assembly, plugin supervision, built-in tools, runtime handles, or provider-shaped request/response types.

**Rationale:** Loose coupling is the point of making Clankers embeddable. If generic crates start reaching back into Clankers app shells, external users technically can import the crates but still inherit the monolith.

**Alternative:** Provide convenience adapters directly from generic crates into Clankers runtime implementations. Rejected for the SDK core because it inverts ownership. Convenience adapters can exist later as explicitly application-layer crates or modules outside the generic SDK surface.

### 5. API stability is inventory-gated, with explicit support labels

**Choice:** Add a deterministic public API inventory for the supported SDK crates and compare docs/support labels against that inventory. Store the durable inventory as `docs/src/generated/embedded-sdk-api.md`, generated or checked by the acceptance bundle from a focused source/API scan.

**Rationale:** This repo is moving fast. A support policy without an inventory would be aspirational. An inventory gives future changes a concrete review target when they add, remove, or rename supported entrypoints.

**Alternative:** Promise broad crate-level stability. Rejected because many exported items may remain experimental or internal until external use hardens them.

### 6. Feature/default policy is checked against manifests and the minimal example

**Choice:** Document which SDK crates are usable with default features and which optional features are supported for embedding, then validate that statement against Cargo manifests and the minimal fixture build.

**Rationale:** Dependency-light embedding can regress silently through feature defaults. The productization acceptance bundle should catch a default feature pulling in provider, daemon, TUI, DB, or prompt-assembly dependencies.

**Alternative:** Depend only on cargo-tree denylist checks. Rejected because denylist checks prove current graph cleanliness but do not explain which feature combinations embedders should use.

### 7. One acceptance script owns the finish line

**Choice:** Introduce `scripts/check-embedded-agent-sdk.sh` as the documented acceptance bundle. It should compose existing rails (`scripts/check-llm-contract-boundary.sh`, `fcis_shell_boundaries`, agent turn parity) plus new docs/example/API/feature checks. The agent turn parity slice must name and execute focused checks for streaming deltas, tool execution and tool failures, retry/backoff behavior, cancellation behavior, usage observations/final summaries, and terminal stop/error behavior.

**Rationale:** Multiple scattered commands made previous engine status hard to summarize. A named acceptance bundle gives maintainers and future agents one command to run before claiming the embedded SDK is ready.

**Alternative:** Only list commands in docs. Rejected because command lists drift and are harder to use as evidence.

## Risks / Trade-offs

**[Fixture accidentally depends on workspace internals]** → Keep the fixture manifest explicit and run a dependency denylist against that manifest.

**[API inventory becomes noisy]** → Scope stability labels to documented embedding entrypoints first; classify the rest as unsupported/internal until intentionally promoted.

**[Docs overpromise stability]** → Require every supported entrypoint to have a stability classification or migration-note rule.

**[Acceptance script gets too slow]** → Keep it to focused checks and example execution; leave full workspace validation to broader CI.

**[Async executor choice leaks into SDK]** → Treat any executor dependency in the example as application-owned and verify the generic SDK traits do not expose runtime handles as required API parameters.

## Migration Plan

1. Add SDK docs and support/feature policy text.
2. Add the external-consumer fixture and make it run locally through the host runner.
3. Add API inventory, docs/reference, dependency, and feature/default checks.
4. Add `scripts/check-embedded-agent-sdk.sh` that composes the new checks plus existing boundary/parity rails.
5. Capture final validation evidence under the change and keep generated docs/build artifacts fresh.

## Open Questions

None. The implementation should use `examples/embedded-agent-sdk/` for the minimal external-consumer fixture and `docs/src/generated/embedded-sdk-api.md` for the durable SDK API inventory.
