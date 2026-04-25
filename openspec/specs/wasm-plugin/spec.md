# wasm-plugin Specification

## Purpose
Defines which library crates are packaged as Extism WASM plugins and the manifest, tool surface, host-data, and runtime-coverage requirements for those plugins.

## Requirements
### Requirement: Plugin Eligibility Criteria
A crate MUST be packaged as a WASM plugin only when its core logic compiles to `wasm32-unknown-unknown`, its functionality is useful as an LLM-callable tool during an agent session, and the tool semantics fit the plugin SDK request/response model.

#### Scenario: Native-only or type-only crates stay library crates
- GIVEN a candidate crate for WASM plugin packaging
- WHEN it depends on native-only runtime features or only exports compile-time shared types
- THEN it MUST NOT be packaged as a WASM plugin
- AND it SHOULD remain a library crate only

### Requirement: Disqualified Reduced-Scope Crates
The reduced-scope crates `clanker-plugin-sdk`, `clanker-tui-types`, and `clanker-message` MUST NOT be packaged as WASM plugins.

#### Scenario: Reduced-scope non-tool crates are not plugin packaged
- GIVEN the reduced-scope workspace-local crates are assessed for WASM plugin packaging
- WHEN the crate is the plugin SDK itself or only provides compile-time shared types
- THEN no WASM plugin wrapper is created for that crate
- AND the crate remains a workspace-local library crate only

### Requirement: openspec Plugin Architecture
The `openspec` library MUST support both native library use and WASM plugin wrapping. Core parsing and graph logic MUST be separated from filesystem access.

#### Scenario: Native consumer uses filesystem-backed shell
- GIVEN the `openspec` library
- WHEN a consumer uses it natively
- THEN `SpecEngine` handles filesystem access directly via `std::fs`
- AND all existing API methods work unchanged

#### Scenario: WASM plugin receives host-provided data
- GIVEN the `openspec` library
- WHEN a consumer wraps it as a WASM plugin
- THEN tool handlers receive file contents and directory listings as JSON arguments
- AND the plugin returns structured JSON results
- AND no `std::fs` calls happen inside the WASM module

### Requirement: openspec Plugin Tools
The `openspec` WASM plugin MUST expose `spec_list`, `spec_parse`, `change_list`, `change_verify`, and `artifact_status`.

#### Scenario: Valid tool call returns pure-core output
- GIVEN a valid tool call into `openspec-plugin`
- WHEN the plugin handles the request
- THEN it returns structured JSON derived from pure `openspec::core` helpers
- AND it does not read the filesystem directly

### Requirement: openspec Plugin Manifest
The plugin MUST ship with a `plugin.json` manifest that declares plugin name, version, description, Extism runtime kind, the five tool names, the `agent_start` event, and JSON input schemas for each tool.

#### Scenario: Manifest advertises runtime and tool contract
- GIVEN the `openspec-plugin` package is inspected
- WHEN `plugin.json` is read
- THEN the manifest declares the plugin identity and Extism runtime kind
- AND it declares the five tool names and the `agent_start` event
- AND it declares JSON input schemas for each tool

### Requirement: openspec Plugin Runtime Coverage
The plugin MUST have durable checked-in runtime coverage in addition to any ad-hoc smoke scripts.

#### Scenario: Runtime coverage exercises positive and negative calls
- GIVEN the `openspec-plugin/tests/runtime.rs` integration test
- WHEN `cargo test --manifest-path openspec-plugin/Cargo.toml` runs
- THEN Extism loads the built plugin module
- AND the test exercises `describe`, `on_event`, and all five tools
- AND both positive and negative tool-call cases are covered
