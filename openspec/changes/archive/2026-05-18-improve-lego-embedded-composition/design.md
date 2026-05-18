## Context

The current embedded-agent SDK is verified and intentionally small: `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clanker-message`, and optional `clankers-core`. The acceptance rail already proves those crates exclude shell concerns such as daemon sockets, TUI, provider discovery, session DB ownership, plugin supervision, Matrix, iroh/P2P, runtime handles, timestamps, and global services.

That boundary is correct, but it leaves product embedders with low-level trait wiring. The next step is not to move app-shell behavior downward; it is to add explicitly shell-free composition pieces above the engine/host/tool contracts.

## Goals / Non-Goals

**Goals:**

- Make the embedded path feel like composable lego bricks.
- Provide reusable, boring host adapters for the most common seams.
- Provide product-starting kits and examples that compile outside the workspace graph.
- Allow declarative tool metadata/capability policy without executing tools at load time.
- Keep one acceptance command that proves the lego layer did not import Clankers shells.

**Non-Goals:**

- Public third-party semver freeze beyond documented supported entrypoints.
- Moving daemon/TUI/provider discovery/OAuth/session DB/plugin supervision into generic SDK crates.
- Product-specific web/desktop/cloud integrations.
- Replacing daemon/MCP/ACP sidecar integration paths.

## Decisions

### 1. Add adapter bricks as a separate shell-free crate

**Choice:** Introduce a reusable adapter-brick crate, expected name `clankers-adapters`, that depends only on the embedded SDK crates and shell-free utilities.

**Rationale:** Product embedders repeatedly need memory event collection, cancellation, retry sleeper, usage observer, and fake/scripted model behavior. Providing these in a dedicated crate makes them reusable while preserving dependency-inversion.

**Alternative rejected:** Put these adapters in `clankers-engine-host`. That would blur the line between trait contracts and convenience implementations, making it easier for host-shell dependencies to creep into the core runner.

**Implementation:** Add structs such as `MemoryEventSink`, `AtomicCancellationSource`, `NoopRetrySleeper` or deterministic sleeper, `CollectingUsageObserver`, and `FakeModelHost`/`ScriptedModelHost`. Each should expose observations suitable for tests without live credentials.

### 2. Treat kits as compositions, not new authoritative policy

**Choice:** Composition kits assemble existing SDK crates and adapter bricks. They do not own turn policy, prompt lifecycle policy, provider discovery, or tool execution semantics.

**Rationale:** The engine remains authoritative for model/tool turn policy. Kits should reduce boilerplate, not fork behavior.

**Alternative rejected:** A monolithic `EmbeddedAgent` that hides all seams and grows into another app shell. That would recreate the current terminal agent as a library and weaken product control.

**Implementation:** Start with minimal and tool-enabled recipes. If a crate/module exposes kit constructors, require tests proving replaceability of model/tool/event/cancellation components.

### 3. Tool catalogs are metadata-first and execution-free

**Choice:** Declarative tool catalogs load into typed metadata and validation outcomes only; loading a catalog must not start runtimes or execute tools.

**Rationale:** Product startup should be safe and deterministic. Catalogs should describe capabilities and schemas, while application-owned runtimes decide execution.

**Alternative rejected:** Reusing plugin discovery as the catalog loader. Plugin discovery has runtime/supervision concerns and is outside the generic embedded path.

**Implementation:** Define typed catalog DTOs and validation errors. Serialized formats can be added behind parsers, but public semantics should not require a specific parser.

### 4. Capability packs are snapshot-tested safety contracts

**Choice:** Named packs expose stable capability sets or snapshots. Dangerous packs require explicit opt-in and documentation.

**Rationale:** Safe defaults are a central part of lego-like composition. Silent expansion of a preset is a production risk.

**Alternative rejected:** Free-form string lists only. They are flexible but do not create durable safety evidence.

**Implementation:** Add focused matrix/snapshot tests for each preset and a negative test that demonstrates dangerous capabilities are not included in minimal defaults.

### 5. Extend the existing embedded acceptance rail

**Choice:** `scripts/check-embedded-agent-sdk.sh` remains the one command for embedded readiness and expands to cover adapters, kits, catalogs, capability packs, and recipes.

**Rationale:** A single rail prevents fragmented readiness claims and keeps docs/examples/API inventory in lockstep.

**Alternative rejected:** Separate ad hoc checks per crate/example. That would let lego readiness drift from embedded SDK readiness.

**Implementation:** Add dependency denylist checks for new crates/examples, executable recipe runs, catalog validation tests, capability-pack snapshots, and docs/API inventory freshness.

## Risks / Trade-offs

**Adapter crate becomes a dumping ground** → Mitigate with dependency denylist, source-token checks, and a rule that adapter bricks only implement existing host/tool traits or typed catalog helpers.

**Kits hide too much and block product replacement** → Mitigate with replaceability tests and docs that show overriding each seam.

**Catalog schema becomes too broad** → Start with metadata needed by `ToolCatalog`: name, description, schema, capability, approval, runtime kind, redaction. Add fields only with fail-closed validation and tests.

**Capability pack names imply stronger security than implemented** → Treat packs as policy presets, not a sandbox. Docs must name required runtime enforcement points and dangerous opt-ins.

**Acceptance rail gets slow** → Keep examples small, fake credential-free, and deterministic; run broad workspace checks separately from the focused embedded rail.

## Validation Plan

- `scripts/check-embedded-agent-sdk.sh`
- Focused crate tests for adapter observations and replacement behavior.
- Tool catalog positive and negative validation tests.
- Capability-pack snapshot/matrix tests.
- Executable examples outside the workspace graph for minimal, tool-enabled, and fail-closed recipe paths.
- `cargo check --workspace --all-targets` before landing implementation slices.
