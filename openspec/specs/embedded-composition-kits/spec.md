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

### Requirement: Product-workbench embedded dogfood [r[embedded-composition-kits.product-workbench]]

The system MUST provide a checked product-style embedded dogfood recipe that composes product-owned session storage, product-owned provider adaptation, and product-owned tool execution in one in-process integration while preserving generic SDK boundaries.

#### Scenario: Combined seams run through green SDK crates [r[embedded-composition-kits.product-workbench.combined-seams]]

- GIVEN a product-style workbench example composes an embedded agent from documented SDK crates
- WHEN the example runs its first turn and a restored follow-up turn
- THEN it MUST route model execution through a product-owned `ModelHost` adapter, product tools through `EmbeddedToolCatalog`/`CatalogToolExecutor`, and persistence through product-owned session/message/receipt DTOs
- THEN it MUST NOT import Clankers daemon sockets, TUI/rendering crates, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles

#### Scenario: Product-workbench persists and restores context [r[embedded-composition-kits.product-workbench.example]]

- GIVEN the product-workbench example runs an initial tool-using turn
- WHEN it persists the resulting transcript and reloads the same product-owned session for a follow-up prompt
- THEN the follow-up model request MUST include the prior user/tool/assistant context and the new prompt in deterministic order
- THEN the example MUST persist a product-owned turn receipt that records session id, turn index, model request count, tool call summaries, and usage totals

#### Scenario: Product-workbench fails closed [r[embedded-composition-kits.product-workbench.fail-closed]]

- GIVEN the product-workbench example receives a missing session id or a catalog entry requiring dangerous capabilities without approval
- WHEN the recipe attempts to run that path
- THEN missing-session handling MUST return an explicit product-owned error before model/tool execution and MUST NOT create a replacement session
- THEN dangerous-tool handling MUST deny execution before product tool code runs and MUST expose the denial as deterministic recipe evidence

### Requirement: Real product dogfood integration [r[embedded-composition-kits.real-product-dogfood]]

The system MUST prove lego-style Clankers composition in at least one real product integration before promoting more generic SDK API.

#### Scenario: Product consumes only green SDK bricks [r[embedded-composition-kits.real-product-dogfood.green-surface]]

- GIVEN a selected product integrates an embedded Clankers agent
- WHEN its dependency graph and source imports are checked
- THEN it MUST use only documented green generic SDK crates for in-process engine composition
- THEN it MUST NOT import daemon sockets, TUI/rendering crates, provider discovery, OAuth stores, Clankers session DB ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles

#### Scenario: Product dogfood manifest is checked [r[embedded-composition-kits.real-product-dogfood.product-dogfood-manifest-is-checked]]

- GIVEN the embedded SDK lego-readiness rail is being advanced
- WHEN dogfood runtime evidence is accepted
- THEN a selected product embedding MUST declare its SDK crate set, capability packs, tool catalog references, provider/session seams, and shell exclusions in a checked manifest first
- THEN the manifest MUST be validated by the lego policy checker and the real-product dogfood checker before generated transcript evidence is trusted

#### Scenario: Dogfood evidence is content addressed [r[embedded-composition-kits.real-product-dogfood.receipt]]

- GIVEN the product dogfood rail completes
- WHEN it emits a receipt
- THEN the receipt MUST include BLAKE3 hashes for the dogfood manifest, dependency-boundary report, executable recipe or integration test source, and sanitized runtime transcript
- THEN changing any hashed evidence artifact MUST change the receipt without requiring live credentials or network access

#### Scenario: Dogfood run emits reproducible transcript evidence [r[embedded-composition-kits.real-product-dogfood.dogfood-run-emits-reproducible-transcript-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced
- WHEN the product dogfood checker runs
- THEN the dogfood rail MUST emit a sanitized transcript plus dependency-boundary report and BLAKE3 receipt without live credentials, network access, daemon startup, provider discovery, OAuth stores, or user-local state

#### Scenario: Product manifest is contract checked [r[embedded-composition-kits.real-product-dogfood.nickel-manifest]]

- GIVEN the integration declares its embedded-agent composition in a checked manifest
- WHEN the manifest is exported for the dogfood rail
- THEN Nickel contracts SHOULD validate selected crates, capability packs, tool catalog references, and forbidden shell surfaces before Rust tests run
- THEN the Rust runtime MUST consume exported typed data or generated fixtures rather than evaluating Nickel inside generic SDK crates

#### Scenario: Dogfood findings drive brick backlog [r[embedded-composition-kits.real-product-dogfood.dogfood-findings-drive-brick-backlog]]

- GIVEN the dogfood integration needs app-owned glue that appears reusable across products
- WHEN a developer wants to promote that glue into a green SDK dependency or public brick
- THEN the result MUST be recorded as a follow-up OpenSpec rather than silently expanding green SDK dependencies

### Requirement: Embedded brick contract stability [r[embedded-composition-kits.brick-contracts]]

The system MUST define stable, inspectable contracts for product-facing embedded SDK bricks.

#### Scenario: Green/yellow/red boundary is contract checked [r[embedded-composition-kits.brick-contracts.boundary-policy]]

- GIVEN product docs classify crates as green generic SDK bricks, yellow app-edge surfaces, or red shell/runtime internals
- WHEN the embedded SDK acceptance rail runs
- THEN the classification MUST be generated from or checked against a typed policy contract
- THEN red shell/runtime crates MUST be denied from green SDK examples and adapter crates with actionable diagnostics

#### Scenario: Public brick API inventory is stable evidence [r[embedded-composition-kits.brick-contracts.api-inventory]]

- GIVEN a green SDK crate exposes product-facing public API
- WHEN the API inventory checker runs
- THEN it MUST detect added, removed, or renamed public brick items that affect embedder semver expectations
- THEN the release receipt MUST include BLAKE3 hashes and byte sizes for generated API inventory artifacts

#### Scenario: Contract changes are reviewed explicitly [r[embedded-composition-kits.brick-contracts.change-control]]

- GIVEN a patch changes the crate boundary policy, denylist, or public brick inventory baseline
- WHEN verification runs
- THEN it MUST fail until docs, expected inventory, and receipt evidence are intentionally updated together
- THEN Nickel contract exports SHOULD be checked in or reproducibly generated by a deterministic script

#### Scenario: Supported brick inventory is explicit [r[embedded-composition-kits.brick-contracts.supported-brick-inventory-is-explicit]]

- GIVEN the embedded SDK lego-readiness rail is being advanced
- WHEN this slice is implemented and verified
- THEN Every documented green SDK entrypoint maps to an exported Rust item or checked example path and is classified as supported, compatibility alias, or internal/non-contract.

#### Scenario: Breaking brick drift requires migration evidence [r[embedded-composition-kits.brick-contracts.breaking-brick-drift-requires-migration-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced
- WHEN this slice is implemented and verified
- THEN Removing, renaming, or semantically repurposing a supported brick fails verification until docs, migration notes, examples, and receipt evidence are updated together.

#### Scenario: Boundary policy stays in generated evidence [r[embedded-composition-kits.brick-contracts.boundary-policy-stays-in-generated-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced
- WHEN this slice is implemented and verified
- THEN The release receipt includes hashes and byte sizes for API inventory, docs, checker policy, and examples so downstream embedders can audit the exact brick contract they consumed.

### Requirement: Composable capability-pack contracts [r[embedded-composition-kits.capability-pack-composition]]

The system MUST let embedders compose capability packs while preserving fail-closed safety boundaries.

#### Scenario: Pack merge order is deterministic [r[embedded-composition-kits.capability-pack-composition.merge]]

- GIVEN a product selects multiple capability packs
- WHEN the packs are merged
- THEN the resulting ordered capability set MUST be deterministic and snapshot-tested
- THEN conflicts such as safe-pack plus dangerous override MUST produce typed diagnostics unless an explicit approval policy permits the combination

#### Scenario: Dangerous conflicts fail closed [r[embedded-composition-kits.capability-pack-composition.dangerous-conflicts]]

- GIVEN a product selects multiple capability packs
- WHEN the selected set combines a safe-pack with shell, network, secret-adjacent, or other dangerous expansion
- THEN the merge MUST fail with typed diagnostics unless a product-owned approval policy explicitly allows the dangerous combination
- THEN denied combinations MUST NOT silently widen the resulting capability set

#### Scenario: Pack policy is checked before Rust use [r[embedded-composition-kits.capability-pack-composition.nickel-policy]]

- GIVEN capability packs are declared in a data-oriented policy file
- WHEN the policy is exported
- THEN Nickel contracts SHOULD validate pack names, capability atoms, danger class, merge priority, default status, and required human-approval labels
- THEN generic SDK crates MUST consume typed Rust data or generated fixtures rather than depending on Nickel at runtime

#### Scenario: Safety snapshots are content addressed [r[embedded-composition-kits.capability-pack-composition.blake3-snapshots]]

- GIVEN the acceptance rail evaluates capability-pack presets and composed packs
- WHEN it emits evidence
- THEN it MUST include BLAKE3 hashes for exported pack policy, exact allowed-capability snapshots, and dangerous-capability denial fixtures
- THEN a silent expansion of a safe pack MUST change evidence and fail focused tests unless docs and expected snapshots are updated

### Requirement: Tool catalog manifest contract [r[embedded-composition-kits.tool-catalog-manifest]]

The system MUST support a product-owned tool catalog manifest that validates tools before they are exposed to an embedded agent.

#### Scenario: Manifest export is normalized and runtime-neutral [r[embedded-composition-kits.tool-catalog-manifest.export]]

- GIVEN a product authors a declarative tool catalog manifest
- WHEN the manifest is validated and exported
- THEN it MUST produce runtime-neutral embedded tool metadata compatible with `EmbeddedToolCatalog`
- THEN export MUST NOT start stdio processes, load Extism modules, perform network calls, open secrets, or execute product tools

#### Scenario: Manifest validation diagnostics are actionable [r[embedded-composition-kits.tool-catalog-manifest.fail-closed]]

- GIVEN a catalog contains duplicate names, invalid schemas, unknown runtime kinds, unsafe defaults, missing redaction policy, or undeclared dangerous capabilities
- WHEN validation runs
- THEN it MUST return typed errors before metadata is visible to an agent turn
- THEN denied fields MUST NOT be silently dropped or widened

#### Scenario: Normalized evidence distinguishes semantic drift [r[embedded-composition-kits.tool-catalog-manifest.blake3-evidence]]

- GIVEN the acceptance rail validates catalog fixtures
- WHEN it records catalog evidence
- THEN it MUST include BLAKE3 hashes for authored manifests, normalized exported metadata, denial fixtures, and truncation fixtures
- THEN non-semantic formatting changes MAY avoid changing normalized metadata hashes, but semantic policy changes MUST change the evidence

#### Scenario: Nickel remains an authoring boundary [r[embedded-composition-kits.tool-catalog-manifest.nickel-authoring]]

- GIVEN Nickel is used to author the first-class catalog manifest
- WHEN embedded Rust crates load catalog data
- THEN Nickel SHOULD provide author-time contracts and exported fixture data
- THEN generic SDK crates MUST NOT require Nickel evaluation, filesystem policy files, or shell commands at runtime

### Requirement: Provider adapter kit polish [r[embedded-composition-kits.provider-adapter-kit]]

The system MUST make product-owned model-provider adaptation easy to copy without importing Clankers provider runtime shells.

#### Scenario: Provider adapter recipe covers outcome classes [r[embedded-composition-kits.provider-adapter-kit.outcomes]]

- GIVEN a product-owned `ModelHost` adapter recipe is checked into examples
- WHEN the recipe runs
- THEN it MUST demonstrate completed, retryable-failure, terminal-failure, and usage-accounting outcomes
- THEN each outcome MUST be asserted without live credentials, network access, OAuth stores, provider discovery, or router daemon RPC

#### Scenario: Provider adapter template is fixture backed [r[embedded-composition-kits.provider-adapter-kit.provider-adapter-template-is-fixture-backed]]

- GIVEN provider-adapter examples use request/response fixtures
- WHEN verification runs
- THEN fixture inputs and expected normalized outputs MUST be explicit literals or exported data, not produced by the code path under test
- THEN the embedded release receipt SHOULD include BLAKE3 hashes for representative request fixtures, response fixtures, and adapter-run receipts

#### Scenario: Model capability profile remains product-owned [r[embedded-composition-kits.provider-adapter-kit.model-capability-profile-remains-product-owned]]

- GIVEN a product declares model limits, retry policy, or feature flags for its adapter
- WHEN those declarations are contract checked
- THEN optional model limits, retry policy, and feature flags MUST be declared as product-owned data and consumed as typed Rust inputs
- THEN Nickel MAY validate the example profile shape and defaults at author time
- THEN the generic SDK MUST expose Rust traits and DTOs rather than a Nickel-dependent provider abstraction

#### Scenario: Template dependency boundary is enforced [r[embedded-composition-kits.provider-adapter-kit.template-dependency-boundary-is-enforced]]

- GIVEN a product-owned provider adapter template is checked into examples
- WHEN the embedded SDK acceptance rail runs
- THEN the template and examples MUST reject `clankers-provider`, clanker-router daemon RPC, OAuth stores, provider discovery, and live network credentials from the generic SDK path

### Requirement: Session/resume brick convergence [r[embedded-composition-kits.session-resume-brick]]

The system MUST gather comparable host-owned session/resume evidence before promoting a reusable public session API.

#### Scenario: Multiple product-shaped stores prove restored context [r[embedded-composition-kits.session-resume-brick.multiple-product-shaped-stores-prove-restored-context]]

- GIVEN at least two product-style embeddings persist and resume embedded sessions
- WHEN their follow-up turns run through the embedded SDK acceptance rail
- THEN each MUST prove restored user/tool/assistant context reaches the next `EngineModelRequest` in deterministic order
- THEN each product MUST own its storage DTOs and persistence I/O unless a later OpenSpec promotes a reusable session trait

#### Scenario: Missing and stale sessions fail closed [r[embedded-composition-kits.session-resume-brick.missing-and-stale-sessions-fail-closed]]

- GIVEN a product requests a missing, stale, or fork-prone session id
- WHEN restore is attempted
- THEN the embedding MUST fail closed before model/tool execution or explicitly create a new session through a separate product-owned path
- THEN it MUST NOT silently fork a replacement session or read Clankers JSONL/DB/session shell state

#### Scenario: Reusable API promotion waits for convergence [r[embedded-composition-kits.session-resume-brick.reusable-api-promotion-waits-for-convergence]]

- GIVEN multiple product embeddings expose similar session DTO or store shapes
- WHEN a developer wants to promote that shape into a generic green SDK API
- THEN the repeated shape MUST be recorded as convergence evidence for a later OpenSpec
- THEN the current generic SDK MUST NOT import `clankers-session`, `clankers-db`, Clankers JSONL restore shells, daemon sockets, or TUI/session restore logic as part of this evidence-only slice

#### Scenario: Resume evidence is content addressed [r[embedded-composition-kits.session-resume-brick.blake3-evidence]]

- GIVEN a product emits session/resume evidence
- WHEN the dogfood rail completes
- THEN it SHOULD include BLAKE3 hashes for sanitized transcripts, restored-context fixtures, session DTO schema examples, and turn receipts
- THEN privacy-sensitive data MUST be redacted before hashing when evidence is committed

#### Scenario: Schema contracts are optional authoring aids [r[embedded-composition-kits.session-resume-brick.nickel-schema]]

- GIVEN a product wants a checked session DTO schema or migration policy
- WHEN it authors one in Nickel
- THEN Nickel contracts MAY validate product-owned schema examples and migration fields
- THEN Clankers generic SDK crates MUST NOT take ownership of product persistence, migrations, or database access

### Requirement: Plugin/tool runtime separation [r[embedded-composition-kits.plugin-tool-runtime-separation]]

The system MUST keep tool runtime kinds swappable behind explicit contracts without sending one runtime kind through another runtime loader.

#### Scenario: Runtime kind dispatch is explicit [r[embedded-composition-kits.plugin-tool-runtime-separation.dispatch]]

- GIVEN a catalog or plugin manifest declares a runtime kind such as Extism, stdio, built-in, or product-owned executor
- WHEN discovery and execution planning run
- THEN only the matching runtime loader/executor MAY receive that entry
- THEN non-Extism entries MUST NOT flow through eager WASM loading or produce bogus missing-WASM errors

#### Scenario: Launch policy is contract checked [r[embedded-composition-kits.plugin-tool-runtime-separation.nickel-policy]]

- GIVEN runtime manifests include launch policy, sandbox requirements, capability requirements, and redaction policy
- WHEN manifest policy is checked
- THEN Nickel contracts SHOULD validate runtime-kind allowlists, required fields per kind, and bounded exceptions before runtime dispatch
- THEN generic SDK crates MUST consume typed manifest data rather than depending on Nickel evaluation at execution time

#### Scenario: Dispatch matrix evidence is content addressed [r[embedded-composition-kits.plugin-tool-runtime-separation.blake3-matrix]]

- GIVEN tests cover Extism, stdio, built-in, and product-owned tool entries
- WHEN the acceptance rail records runtime dispatch evidence
- THEN it SHOULD include BLAKE3 hashes for normalized manifests, runtime-kind allowlist exports, and dispatch matrix fixtures
- THEN changing the runtime-kind contract MUST require an intentional update to tests, docs, and receipt evidence
