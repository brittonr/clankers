# WASM Plugin Packaging — Spec

## Purpose

Defines when and how a reduced-scope extracted crate should also ship a WASM
plugin wrapper. Within `crate-extraction-2`, only `openspec` qualifies.

## Requirements

### Plugin Eligibility Criteria

A crate SHOULD be packaged as a WASM plugin only when ALL of:

1. its core logic compiles to `wasm32-unknown-unknown`
2. its functionality is useful as an LLM-callable tool during an agent session
3. the tool semantics fit the plugin SDK request/response model

GIVEN a candidate crate for WASM plugin packaging
WHEN it depends on native-only runtime features or only exports compile-time
     shared types
THEN it MUST NOT be packaged as a WASM plugin
AND it SHOULD be extracted as a library crate only

### Disqualified Reduced-Scope Crates

The following reduced-scope crates MUST NOT be packaged as WASM plugins:

| Crate | Reason |
|---|---|
| clanker-plugin-sdk | It is the SDK itself, not a runtime tool |
| clanker-tui-types | UI event/action types are consumed at compile time |
| clanker-message | Conversation/message types are consumed by agent/runtime code |

### openspec Plugin Architecture

The `openspec` library MUST support both native library use and WASM plugin
wrapping. Core parsing and graph logic MUST be separated from filesystem access.

GIVEN the `openspec` library
WHEN a consumer uses it natively
THEN `SpecEngine` handles filesystem access directly via `std::fs`
AND all existing API methods work unchanged

GIVEN the `openspec` library
WHEN a consumer wraps it as a WASM plugin
THEN tool handlers receive file contents and directory listings as JSON args
AND the plugin returns structured JSON results
AND no `std::fs` calls happen inside the WASM module

### openspec Plugin Tools

The `openspec` WASM plugin MUST expose these tools:

- `spec_list`
- `spec_parse`
- `change_list`
- `change_verify`
- `artifact_status`

GIVEN a valid tool call into `openspec-plugin`
WHEN the plugin handles the request
THEN it returns structured JSON derived from pure `openspec::core` helpers
AND it does not read the filesystem directly

### openspec Plugin Manifest

The plugin MUST ship with a `plugin.json` manifest that declares:
- plugin name/version/description
- Extism runtime kind
- the five tool names
- the `agent_start` event
- JSON input schemas for each tool

### openspec Plugin Runtime Coverage

The plugin MUST have durable checked-in runtime coverage in addition to any
ad-hoc smoke scripts.

GIVEN `../openspec/openspec-plugin/tests/runtime.rs`
WHEN `cargo test --manifest-path openspec-plugin/Cargo.toml` runs
THEN Extism loads the built plugin module
AND the test exercises `describe`, `on_event`, and all five tools
AND both positive and negative tool-call cases are covered
