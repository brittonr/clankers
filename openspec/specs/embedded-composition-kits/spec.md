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

- GIVEN a preset such as `read_only`, `networkless_coding`, `project_local_edit`, `human_approved_shell`, or `embedding_safe`
- WHEN the preset is converted into capability policy
- THEN tests MUST assert its exact allowed capability set or a stable generated snapshot
- THEN adding a dangerous capability to a preset MUST fail a focused regression test unless the change explicitly updates docs and expected evidence

#### Scenario: Dangerous packs require explicit opt-in [r[embedded-composition-kits.capability-packs.explicit-danger]]

- GIVEN a capability pack can mutate files, run shell/process work, access network, or expose raw logs/secrets
- WHEN a product selects that pack
- THEN the API and docs MUST make the risk explicit
- THEN the default minimal embedding path MUST NOT select that pack implicitly

### Requirement: Executable composition recipes [r[embedded-composition-kits.recipes]]

The system MUST provide checked executable recipes that demonstrate supported lego-style compositions.

#### Scenario: Recipes cover positive and negative paths [r[embedded-composition-kits.recipes.coverage]]

- GIVEN embedded composition recipes are checked into `examples/`
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST compile/run at least a minimal recipe, a tool-enabled recipe, and a negative/fail-closed catalog or capability-policy recipe
- THEN recipe dependency graphs MUST be checked for forbidden shell/runtime dependencies

#### Scenario: Green/yellow/red crate guidance is generated or checked [r[embedded-composition-kits.recipes.crate-guidance]]

- GIVEN product docs describe which Clankers crates are appropriate for product embeddings
- WHEN the embedded SDK acceptance rail runs
- THEN docs MUST classify generic SDK crates as green, app-edge integration crates as yellow, and shell/internal crates as red for generic embedding
- THEN the classification MUST be checked against the actual workspace crate list or an explicit reviewed inventory

### Requirement: Embedded composition acceptance rail [r[embedded-composition-kits.acceptance-rail]]

The system MUST extend the existing embedded SDK acceptance command so lego-style composition claims are verified before readiness is claimed.

#### Scenario: One command verifies lego readiness [r[embedded-composition-kits.acceptance-rail.one-command]]

- GIVEN a developer changes adapter bricks, kits, catalogs, capability packs, or recipes
- WHEN `scripts/check-embedded-agent-sdk.sh` runs
- THEN it MUST verify API inventory freshness, dependency denylist coverage, source-boundary checks, executable recipes, catalog negative cases, capability-pack snapshots, and focused engine/host/tool parity tests
- THEN failure MUST identify the violated lego-boundary rule with enough detail to fix the offending dependency, source token, catalog field, or recipe assertion
