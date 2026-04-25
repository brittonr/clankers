## ADDED Requirements

### Requirement: Productized embedded SDK surface
The system MUST present the reusable engine crates as a documented embedded SDK surface that names supported crates, supported public entrypoints, required host adapters, and excluded Clankers shell concerns.
r[embeddable-agent-engine.productized-sdk-surface]

#### Scenario: embedder can identify the supported crate set
r[embeddable-agent-engine.productized-sdk-surface.supported-crate-set]
- **WHEN** a developer reads the embedded-agent SDK documentation
- **THEN** the documentation names `clankers-engine`, `clankers-engine-host`, `clankers-tool-host`, `clanker-message`, and any required support crates as the supported embedding surface
- **THEN** it clearly states that daemon protocol, TUI rendering, provider discovery, session DB ownership, built-in tool bundles, plugin supervision, and Clankers prompt assembly are not required by the generic embedding path

#### Scenario: public entrypoints are inventoried
r[embeddable-agent-engine.productized-sdk-surface.public-entrypoints-inventoried]
- **WHEN** validation inspects the embedded SDK documentation and public API inventory
- **THEN** each documented entrypoint maps to an actual exported Rust item or example path
- **THEN** stale documentation references fail validation instead of remaining aspirational text

### Requirement: External consumer example
The system MUST include a checked-in external-consumer example or fixture that drives a complete engine turn through the reusable host runner without depending on `clankers-agent` or Clankers application shells.
r[embeddable-agent-engine.external-consumer-example]

#### Scenario: example runs a prompt through fake adapters
r[embeddable-agent-engine.external-consumer-example.fake-adapters]
- **WHEN** validation runs the external-consumer example or fixture
- **THEN** it submits an accepted prompt into `clankers-engine`, executes the resulting turn through `clankers-engine-host::run_engine_turn`, and observes terminal engine output
- **THEN** the model, tool, retry, event, cancellation, and usage adapters are fake, in-memory, or caller-supplied test adapters rather than Clankers provider/daemon/TUI implementations

#### Scenario: example dependency graph excludes Clankers shells
r[embeddable-agent-engine.external-consumer-example.dependency-graph-clean]
- **WHEN** validation inspects the example or fixture dependency graph
- **THEN** the required minimal example or fixture does not depend on `clankers-agent`, `clankers-controller`, `clankers-provider`, `clanker-router`, `clankers-db`, `clankers-protocol`, `clankers-tui`, `clankers-prompts`, `clankers-skills`, `clankers-config`, `clankers-agent-defs`, `ratatui`, `crossterm`, or `iroh`

#### Scenario: example public API avoids runtime-handle leakage
r[embeddable-agent-engine.external-consumer-example.public-api-no-runtime-handles]
- **WHEN** validation inspects the generic embedding crates and example public APIs
- **THEN** they do not expose Tokio runtime handles, network clients, shell-generated message IDs, timestamps, or provider-shaped request/response types as required SDK API parameters

### Requirement: Adapter recipe coverage
The system MUST document and test reusable adapter recipes for model execution, tool execution, retry sleeping, event emission, cancellation, usage observation, and transcript conversion.
r[embeddable-agent-engine.adapter-recipes]

#### Scenario: adapter recipes cover successful and failing paths
r[embeddable-agent-engine.adapter-recipes.positive-negative-paths]
- **WHEN** a host implementer follows the adapter recipes
- **THEN** the recipes show how to return successful model responses, retryable and non-retryable model failures, successful tool results, tool errors, missing-tool results, capability-denied results, cancellation, usage observations, and event sink diagnostics
- **THEN** the recipes point to tests or examples that exercise both positive and negative paths

#### Scenario: transcript conversion stays adapter-owned
r[embeddable-agent-engine.adapter-recipes.transcript-conversion-owned-by-host]
- **WHEN** Clankers-specific persisted messages must be fed into the engine
- **THEN** adapter documentation states that shell-native transcript conversion into `EngineMessage` is host-owned
- **THEN** `clankers-engine` remains free of `AgentMessage` and Clankers shell-only transcript variants

### Requirement: Adapter-only modular coupling
The embedded SDK surface MUST keep engine, host-runner, tool-host, and application concerns loosely coupled through explicit adapter traits, plain data, and dependency-inverted interfaces rather than concrete Clankers runtime implementations.
r[embeddable-agent-engine.adapter-only-modular-coupling]

#### Scenario: host runner depends on interfaces rather than implementations
r[embeddable-agent-engine.adapter-only-modular-coupling.host-runner-traits]
- **WHEN** a host drives an engine turn through the generic host runner
- **THEN** model execution, tool execution, retry sleeping, event emission, cancellation, and usage observation are supplied through trait/interface implementations
- **THEN** the generic host runner does not instantiate or require Clankers provider, daemon, TUI, DB, prompt-assembly, plugin-supervision, or built-in-tool implementations

#### Scenario: composition happens at application edge
r[embeddable-agent-engine.adapter-only-modular-coupling.application-edge-composition]
- **WHEN** an embedder assembles a complete agent
- **THEN** concrete providers, tools, storage, prompts, events, and cancellation sources are wired at the embedder/application edge
- **THEN** `clankers-engine`, `clankers-engine-host`, and `clankers-tool-host` remain reusable modules with no hidden global state, singleton service lookup, or direct shell dependency required for the minimal embedding path

#### Scenario: boundary rails reject tight coupling regressions
r[embeddable-agent-engine.adapter-only-modular-coupling.tight-coupling-rail]
- **WHEN** validation inventories SDK crate dependencies, source imports, public APIs, and the minimal external-consumer fixture
- **THEN** any direct dependency from generic SDK crates to Clankers shell/runtime crates, provider discovery, daemon/TUI types, session DB types, prompt-assembly crates, runtime handles, or provider-shaped request/response types fails validation unless it is isolated in a documented application-layer adapter outside the generic SDK crates

### Requirement: SDK support and versioning policy
The system MUST define the support policy for the embedded SDK surface before presenting it as ready for external consumers.
r[embeddable-agent-engine.sdk-support-policy]

#### Scenario: versioning and migration policy is documented
r[embeddable-agent-engine.sdk-support-policy.versioning-documented]
- **WHEN** a developer reads the embedded-agent SDK documentation
- **THEN** it states the crate versioning source, compatibility expectations, deprecation process, and migration-note location for documented embedding entrypoints
- **THEN** unsupported internal crates, experimental APIs, and application-layer adapters are labeled so consumers do not treat them as stable SDK surface

#### Scenario: support policy is checked against public API inventory
r[embeddable-agent-engine.sdk-support-policy.inventory-classification]
- **WHEN** validation inspects the SDK public API inventory
- **THEN** every documented supported entrypoint has a stability classification or migration-note requirement
- **THEN** unsupported or internal-only items are not advertised as stable embedding API

### Requirement: SDK feature and default policy
The system MUST define and verify feature flags and default-feature expectations for the embedded SDK crates.
r[embeddable-agent-engine.sdk-feature-default-policy]

#### Scenario: feature policy is documented
r[embeddable-agent-engine.sdk-feature-default-policy.documented]
- **WHEN** a developer reads the embedded-agent SDK documentation
- **THEN** it states which SDK crates are usable with default features, which optional features are supported for embedding, and which features are application-layer or experimental
- **THEN** the minimal embedding path does not require enabling Clankers daemon, TUI, provider-discovery, DB, prompt-assembly, plugin-supervision, or built-in-tool features

#### Scenario: feature policy is validated
r[embeddable-agent-engine.sdk-feature-default-policy.validated]
- **WHEN** validation runs the embedded SDK acceptance bundle
- **THEN** it checks the documented default-feature and optional-feature expectations against Cargo manifests and at least one minimal example build
- **THEN** undocumented feature requirements fail validation

### Requirement: Embedding API stability rails
The system MUST keep validation rails that detect breaking or accidental changes to the supported embedding surface before the SDK is presented as ready.
r[embeddable-agent-engine.embedding-api-stability-rails]

#### Scenario: public API inventory is checked
r[embeddable-agent-engine.embedding-api-stability-rails.public-api-inventory]
- **WHEN** validation runs for the embedded SDK surface
- **THEN** it records or checks the public API inventory for `clankers-engine`, `clankers-engine-host`, and `clankers-tool-host`
- **THEN** additions, removals, renames, or signature changes that affect documented embedding entrypoints require an explicit task, release note, or migration note

#### Scenario: dependency boundary stays clean
r[embeddable-agent-engine.embedding-api-stability-rails.dependency-boundary-clean]
- **WHEN** validation checks normal dependency graphs and source imports for the embedded SDK crates
- **THEN** runtime shell, provider, router, daemon, TUI, database, networking, timestamp, shell-generated ID, and async-runtime implementation dependencies remain excluded from the generic embedding crates
- **THEN** failure blocks acceptance of the productization change

### Requirement: Embedding acceptance bundle
The system MUST provide a single documented validation bundle that proves docs, examples, dependency rails, public API inventory, and Clankers adapter parity are fresh for the embedded SDK surface.
r[embeddable-agent-engine.embedding-acceptance-bundle]

#### Scenario: acceptance bundle covers docs and executable examples
r[embeddable-agent-engine.embedding-acceptance-bundle.docs-examples]
- **WHEN** maintainers run the embedded SDK acceptance bundle
- **THEN** it verifies the external-consumer example or fixture, docs links/API references, generated artifact freshness, and dependency/source boundary rails
- **THEN** it produces durable evidence under the change before any implementation tasks are marked done

#### Scenario: acceptance bundle preserves existing Clankers behavior
r[embeddable-agent-engine.embedding-acceptance-bundle.clankers-parity]
- **WHEN** the acceptance bundle validates Clankers integration
- **THEN** it includes focused parity checks proving `clankers-agent::Agent` still routes through the reusable host runner and preserves streaming, tool, retry, cancellation, usage, and terminal behavior for the default Clankers assembly
