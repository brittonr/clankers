# embedded-composition-kits Specification

## Purpose

Define the product-facing composition layer that makes Clankers' embeddable SDK behave like small reusable lego bricks while preserving the existing functional-core / imperative-shell boundaries.
## Requirements
### Requirement: Shell-free adapter bricks for embedding [r[embedded-composition-kits.adapter-bricks]]

The system MUST provide reusable host-adapter bricks for common embedded-agent concerns without importing Clankers shell/runtime implementations.

#### Scenario: Minimal adapter crate excludes shell dependencies [r[embedded-composition-kits.adapter-bricks.shell-free]]

- GIVEN a product depends on the adapter-brick crate for an embedded agent
- WHEN the dependency graph is checked through the embedded SDK acceptance rail
- THEN it MUST exclude daemon protocol clients, TUI/rendering crates, provider discovery, router daemon RPC, session database ownership, Matrix, iroh/P2P, plugin supervision, and built-in Clankers tool bundles
- THEN it MAY depend on documented SDK crates such as `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clanker-message`, and app-neutral utility crates

#### Scenario: Boring adapters cover common host seams [r[embedded-composition-kits.adapter-bricks.common-seams]]

- GIVEN an embedder wants a minimal local embedded agent
- WHEN it constructs host adapters from reusable bricks
- THEN the system SHOULD provide ready-to-use implementations or constructors for an in-memory event sink, cancellation source, retry sleeper, usage observer, and fake/scripted model host
- THEN each adapter MUST expose deterministic test hooks or recorded observations so product tests can assert turn behavior without live credentials

#### Scenario: Adapter bricks remain replaceable [r[embedded-composition-kits.adapter-bricks.replaceable]]

- GIVEN a product replaces one reusable adapter brick with an application-owned implementation
- WHEN the embedded turn is executed
- THEN the replacement MUST satisfy the same `clankers-engine-host` or `clankers-tool-host` trait contract without requiring changes to `clankers-engine`
- THEN the reusable kit MUST NOT hide global singleton services that prevent replacement

### Requirement: Product composition kits [r[embedded-composition-kits.product-kits]]

The system MUST define documented composition kits that assemble SDK crates and adapter bricks into product-starting points while keeping product-owned I/O at the application edge.

#### Scenario: Minimal kit runs without Clankers app shell [r[embedded-composition-kits.product-kits.minimal]]

- GIVEN a checked example outside the workspace crate graph depends on the minimal kit
- WHEN the example runs
- THEN it MUST execute at least one successful embedded turn using product-owned/fake model execution
- THEN it MUST NOT start a daemon, create a TUI, discover providers, read OAuth stores, open the Clankers session DB, supervise plugins, or depend on Matrix/iroh/P2P

#### Scenario: Tool-enabled kit composes explicit tools [r[embedded-composition-kits.product-kits.tool-enabled]]

- GIVEN a checked example depends on a tool-enabled kit
- WHEN the model requests a declared product-owned tool
- THEN the kit MUST route the call through `ToolExecutor`/`ToolCatalog` contracts and feed correlated results back into the engine
- THEN missing tool, tool error, capability denial, and truncation paths MUST be covered by tests or executable recipe assertions

#### Scenario: Daemon-controlled kit stays an app-edge choice [r[embedded-composition-kits.product-kits.daemon-edge]]

- GIVEN a product chooses to control Clankers through daemon/MCP/ACP instead of in-process SDK embedding
- WHEN docs describe that path
- THEN the path MUST be labeled as an app-edge integration surface, not part of the generic embedded SDK compatibility promise
- THEN generic kits MUST remain usable without daemon/MCP/ACP dependencies

### Requirement: Declarative embedded tool catalogs [r[embedded-composition-kits.tool-catalogs]]

The system MUST support a small validated declarative tool-catalog representation for embedded products and convert it into runtime tool metadata without executing tools during catalog loading.

#### Scenario: Catalog maps data to tool metadata [r[embedded-composition-kits.tool-catalogs.metadata]]

- GIVEN a product supplies a declarative catalog containing tool names, descriptions, schemas, capability requirements, approval policy, runtime kind, and redaction policy
- WHEN the catalog is parsed and validated
- THEN the result MUST produce deterministic `ToolCatalog`-compatible metadata
- THEN parsing MUST NOT start stdio processes, load Extism modules, perform network calls, or execute product tools

#### Scenario: Catalog validation fails closed [r[embedded-composition-kits.tool-catalogs.fail-closed]]

- GIVEN a catalog contains duplicate tool names, schema errors, unknown runtime kinds, unsafe capability defaults, or unsupported approval/redaction policy
- WHEN validation runs
- THEN the system MUST return typed validation errors before tool metadata is exposed to an agent turn
- THEN denied catalogs MUST NOT silently drop unsafe fields or widen capabilities

#### Scenario: Catalog format remains implementation-neutral [r[embedded-composition-kits.tool-catalogs.implementation-neutral]]

- GIVEN the first implementation uses Nickel, JSON, TOML, or another checked-in representation
- WHEN an embedder consumes the catalog API
- THEN public semantics MUST be expressed as typed Rust data and validation outcomes rather than requiring callers to depend on a specific file parser
- THEN docs MUST name which serialized formats are supported for the current release

### Requirement: Named capability-pack presets for embeddings [r[embedded-composition-kits.capability-packs]]

The system MUST provide safe named capability-pack presets that embedders can select and test deterministically.

#### Scenario: Safe presets do not expand unexpectedly [r[embedded-composition-kits.capability-packs.no-expansion]]

- GIVEN product-facing presets named `embedding_safe`, `read_only`, `networkless_coding`, `project_local_edit`, and `human_approved_shell`
- WHEN each preset is converted into its ordered embedded capability set
- THEN focused tests MUST assert the exact allowed capability set for every preset
- THEN `embedding_safe`, `read_only`, and `networkless_coding` MUST NOT include explicit opt-in capabilities such as mutate, shell, network, raw-log, or secret-adjacent access unless the expected snapshot and docs are intentionally updated
- THEN adding a dangerous capability to a safe preset MUST fail a focused regression test unless the change explicitly updates docs and expected evidence

#### Scenario: Dangerous packs require explicit opt-in [r[embedded-composition-kits.capability-packs.explicit-danger]]

- GIVEN a capability pack can mutate files, run shell/process work, access network, expose raw logs, or work near secrets
- WHEN a product selects that pack
- THEN the API and docs MUST make the risk explicit through the preset name or description
- THEN `human_approved_shell` MUST be treated as an explicit opt-in pack rather than a default minimal embedding preset
- THEN the default minimal embedding path MUST NOT select that pack implicitly

### Requirement: Executable composition recipes [r[embedded-composition-kits.recipes]]

The system MUST provide checked executable recipes that demonstrate supported lego-style compositions, including product-owned model, tool, and session-storage seams.

#### Scenario: Recipes cover positive and negative paths [r[embedded-composition-kits.recipes.coverage]]

- GIVEN embedded composition recipes are checked into `examples/`
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST compile/run at least a minimal recipe, a tool-enabled recipe, a product-owned provider-adapter recipe, and a negative/fail-closed catalog or capability-policy recipe
- THEN recipe dependency graphs MUST be checked for forbidden shell/runtime dependencies

#### Scenario: Session-store recipe preserves restored context [r[embedded-composition-kits.recipes.session-store-restores-context]]

- GIVEN a standalone embedded session-store recipe uses product-owned session/message DTOs and an app-owned in-memory store
- WHEN the recipe runs one embedded turn, persists the resulting transcript, reloads the session, and runs a follow-up turn
- THEN the follow-up `EngineModelRequest` MUST include the restored prior user/assistant context and the new follow-up prompt in deterministic order
- THEN the recipe MUST preserve the supplied `session_id` through persistence, reload, and model-host request observation

#### Scenario: Session-store recipe fails closed for missing sessions [r[embedded-composition-kits.recipes.session-store-missing-session]]

- GIVEN the product-owned store has no session for a requested id
- WHEN the recipe attempts to restore that session for a follow-up turn
- THEN it MUST return an explicit product-owned missing-session error
- THEN it MUST NOT silently create a replacement session, read Clankers JSONL session files, open `clankers-db`, contact a daemon, or depend on TUI/session restore logic

#### Scenario: Green/yellow/red crate guidance is generated or checked [r[embedded-composition-kits.recipes.crate-guidance]]

- GIVEN product docs describe which Clankers crates are appropriate for product embeddings
- WHEN the embedded SDK acceptance rail runs
- THEN docs MUST classify generic SDK crates as green, app-edge integration crates as yellow, and shell/internal crates as red for generic embedding
- THEN the classification MUST state that product-owned storage/session DTOs are a yellow app-edge concern unless and until a separate OpenSpec promotes a reusable storage API
- THEN the classification MUST be checked against the actual workspace crate list or an explicit reviewed inventory

### Requirement: Embedded composition acceptance rail [r[embedded-composition-kits.acceptance-rail]]

The system MUST extend the existing embedded SDK acceptance command so lego-style composition claims are verified before readiness is claimed.

#### Scenario: One command verifies lego readiness [r[embedded-composition-kits.acceptance-rail.one-command]]

- GIVEN a developer changes adapter bricks, kits, catalogs, capability packs, provider/session recipes, or embedded SDK docs
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST verify API inventory freshness, dependency denylist coverage, source-boundary checks, executable recipes, catalog negative cases, capability-pack snapshots, host-owned session-store recipe behavior, and focused engine/host/tool parity tests
- THEN failure MUST identify the violated lego-boundary rule with enough detail to fix the offending dependency, source token, catalog field, capability-pack preset, session-store assertion, or recipe assertion

### Requirement: Product embedding release receipts [r[embedded-composition-kits.acceptance-rail.release-receipt]]

The system MUST provide a deterministic release receipt for product embedders that captures the embedded SDK readiness boundary and the artifacts used as evidence.

#### Scenario: Receipt records verifiable SDK evidence [r[embedded-composition-kits.acceptance-rail.release-receipt.artifacts]]

- GIVEN a developer runs the embedded SDK acceptance rail or the receipt helper directly
- WHEN the receipt is generated
- THEN it MUST include the current commit identifier, commit date when available, and `git status --short --branch` output
- THEN it MUST include BLAKE3 hashes and byte sizes for the embedded SDK guide, generated API inventory, canonical embedded composition spec, acceptance/check scripts, and standalone embedded examples
- THEN it MUST include the maintained verification commands needed before claiming product embedding readiness

#### Scenario: Receipt preserves green/yellow/red boundaries [r[embedded-composition-kits.acceptance-rail.release-receipt.boundaries]]

- GIVEN a product team reviews the generated receipt before embedding Clankers
- WHEN it inspects the SDK boundary fields
- THEN the receipt MUST identify the green generic SDK crates, yellow app-edge integration surfaces, and red shell/runtime exclusions
- THEN the receipt MUST NOT present daemon, TUI, provider discovery, OAuth stores, session database ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles as generic embedded SDK dependencies

#### Scenario: Acceptance rail emits receipt evidence [r[embedded-composition-kits.acceptance-rail.release-receipt.one-command]]

- GIVEN `scripts/check-embedded-agent-sdk.sh` is the maintained one-command lego readiness rail
- WHEN the command succeeds
- THEN it MUST run the receipt helper and leave a machine-readable receipt under a deterministic target-directory path
- THEN receipt generation MUST NOT add runtime dependencies to the reusable SDK crates or require live credentials, network access, daemon startup, provider discovery, OAuth stores, TUI setup, or Clankers session database access
