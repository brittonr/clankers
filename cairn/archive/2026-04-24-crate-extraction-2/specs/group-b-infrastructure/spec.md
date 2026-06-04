# Group B: Spec Engine Extraction — Spec

## ADDED Requirements

### Requirement: specs Extraction
The `clankers-specs` crate MUST be extracted to `openspec`. The entire spec engine moves: schema, artifacts, changes, specs, delta, merge, verify, templates, and config modules.

#### Scenario: Spec engine API survives extraction
- GIVEN `crates/clankers-specs/` with petgraph and serde_yaml dependencies
- WHEN it is extracted to the `openspec` repo
- THEN `SpecEngine::new()`, `init()`, `discover_specs()`, `discover_changes()`, `create_change()`, `archive_change()`, `sync_change()`, `verify_change()`, and `specs_for_context()` all work
- AND the `Schema`, `Artifact`, `ArtifactGraph`, `Spec`, `Requirement`, `Scenario`, `ChangeInfo`, `TaskProgress`, `DeltaSpec`, `SyncResult`, and `VerifyReport` types are public
- AND the builtin `spec-driven` schema is available via `builtin_spec_driven()`
- AND all `clankers_specs` references are renamed to `openspec`

### Requirement: specs Core/Shell Split
The extracted `openspec` crate MUST separate pure parsing and graph logic from the filesystem-backed shell so the same library can support native use and WASM plugin wrapping.

#### Scenario: Pure core compiles without filesystem feature
- GIVEN the extracted `openspec` repo
- WHEN the crate is compiled with default features disabled
- THEN the pure `openspec::core` parsing, graph, schema, delta, and verification modules compile without `std::fs`
- AND the root crate can still expose `SpecEngine` and config helpers when the `fs` feature is enabled

### Requirement: specs WASM Plugin
In addition to the library extraction, an `openspec-plugin` crate MUST be created that wraps the pure parsing and graph logic as a clankers WASM plugin.

#### Scenario: WASM plugin exposes OpenSpec tools
- GIVEN the `openspec` library crate
- WHEN an `openspec-plugin` crate is built targeting `wasm32-unknown-unknown`
- THEN it exposes `spec_list`, `spec_parse`, `change_list`, `change_verify`, and `artifact_status` via the plugin SDK
- AND it handles `agent_start` events to log readiness
- AND filesystem paths are resolved by the host, not by the WASM module
- AND the plugin compiles with `cargo build --target wasm32-unknown-unknown`

### Requirement: specs WASM Plugin Verification
The `openspec-plugin` crate MUST have durable checked-in runtime coverage that loads the built plugin through Extism and exercises the exported tools.

#### Scenario: Runtime test covers positive and negative plugin calls
- GIVEN the checked-in runtime test suite for `openspec-plugin`
- WHEN `cargo test --manifest-path openspec-plugin/Cargo.toml` runs
- THEN Extism loads the built plugin module
- AND `describe` reports the five tool names
- AND `on_event` handles `agent_start`
- AND each exported tool returns a structured result for valid input
- AND invalid and unknown tool calls fail cleanly without panicking
