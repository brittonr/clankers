# Group B: Spec Engine Extraction — Spec

## Purpose

Defines the extraction contract for the shared spec engine and its WASM plugin
wrapper that moved out of the clankers workspace during `crate-extraction-2`.

## Requirements

### specs Extraction

The `clankers-specs` crate MUST be extracted to `openspec`. The entire
spec engine moves: schema, artifacts, changes, specs, delta, merge, verify,
templates, and config modules.

GIVEN `crates/clankers-specs/` with petgraph + serde_yaml deps
WHEN extracted to the `openspec` repo
THEN `SpecEngine::new()`, `init()`, `discover_specs()`, `discover_changes()`,
    `create_change()`, `archive_change()`, `sync_change()`, `verify_change()`,
    and `specs_for_context()` all work
AND the `Schema`, `Artifact`, `ArtifactGraph`, `Spec`, `Requirement`, `Scenario`,
    `ChangeInfo`, `TaskProgress`, `DeltaSpec`, `SyncResult`, and `VerifyReport`
    types are public
AND the builtin `spec-driven` schema is available via `builtin_spec_driven()`
AND all `clankers_specs` references are renamed to `openspec`

### specs Core/Shell Split

The extracted `openspec` crate MUST separate pure parsing/graph logic from the
filesystem-backed shell so the same library can support native use and WASM
plugin wrapping.

GIVEN the extracted `openspec` repo
WHEN the crate is compiled with default features disabled
THEN the pure `openspec::core` parsing, graph, schema, delta, and verification
    modules compile without `std::fs`
AND the root crate can still expose `SpecEngine` and config helpers when the
    `fs` feature is enabled

### specs WASM Plugin

In addition to the library extraction, an `openspec-plugin` crate MUST be
created that wraps the pure parsing/graph logic as a clankers WASM plugin.

GIVEN the `openspec` library crate
WHEN an `openspec-plugin` crate is built targeting wasm32-unknown-unknown
THEN it exposes these tools via the plugin SDK:
  - `spec_list`
  - `spec_parse`
  - `change_list`
  - `change_verify`
  - `artifact_status`
AND it handles `agent_start` events to log readiness
AND filesystem paths are resolved by the host, not by the WASM module
AND the plugin compiles with `cargo build --target wasm32-unknown-unknown`

### specs WASM Plugin Verification

The `openspec-plugin` crate MUST have durable checked-in runtime coverage that
loads the built plugin through Extism and exercises the exported tools.

GIVEN the checked-in runtime test suite for `openspec-plugin`
WHEN `cargo test --manifest-path openspec-plugin/Cargo.toml` runs
THEN Extism loads the built plugin module
AND `describe` reports the five tool names
AND `on_event` handles `agent_start`
AND each exported tool returns a structured result for valid input
AND invalid and unknown tool calls fail cleanly without panicking
