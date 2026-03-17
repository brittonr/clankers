# WASM Plugin Packaging — Spec

## Purpose

Defines when and how an extracted crate should also ship a WASM plugin
wrapper. Most extracted crates are compile-time infrastructure and SHOULD
NOT be packaged as WASM plugins. This spec identifies the criteria and
the one crate that qualifies.

## Requirements

### Plugin Eligibility Criteria

A crate SHOULD be packaged as a WASM plugin only when ALL of:

1. Its core logic can compile to `wasm32-unknown-unknown` (no mmap, no
   tokio, no native TLS, no FFI)
2. Its functionality is useful as an LLM-callable tool during an agent
   session (not just compile-time types or infrastructure)
3. The tool semantics fit the request/response model of the plugin SDK
   (stateless tool calls, JSON in/out)

GIVEN a candidate crate for WASM plugin packaging
WHEN it uses tokio, redb (mmap), snix (native), matrix-sdk (sqlite/TLS),
     iroh (QUIC/networking), or other native-only dependencies
THEN it MUST NOT be packaged as a WASM plugin
AND it SHOULD be extracted as a library crate only

### Disqualified Crates

The following crates MUST NOT be WASM plugins due to native-only deps:

| Crate | Blocking dependency |
|---|---|
| clanker-nix | snix (nix-compat, snix-eval) — native Nix store interaction |
| clanker-matrix | matrix-sdk (sqlite, vodozemac crypto, rustls) |
| clanker-zellij | iroh (QUIC), tokio |
| clanker-db | redb (mmap, file locks) |
| clanker-protocol | tokio (AsyncRead/AsyncWrite framing) |
| clanker-hooks | tokio, async-trait |

### Disqualified Crates (Wrong Abstraction Level)

The following crates MUST NOT be WASM plugins because their types are
consumed at compile time, not at runtime via tool calls:

| Crate | Reason |
|---|---|
| clanker-tui-types | UI event/action types — consumed by ratatui renderers |
| clanker-message | Conversation message types — consumed by agent core |
| clanker-plugin-sdk | IS the SDK — not a plugin itself |

### openspec Plugin Architecture

The `openspec` library MUST be structured to support both native library
use and WASM plugin wrapping. Core parsing and graph logic MUST be
separated from filesystem access.

GIVEN the openspec library
WHEN a consumer uses it natively (as a Rust dependency)
THEN `SpecEngine` handles filesystem access directly via `std::fs`
AND all existing API methods work unchanged

GIVEN the openspec library
WHEN a consumer wraps it as a WASM plugin
THEN tool handlers receive file contents and directory listings as JSON args
AND the plugin returns structured JSON results
AND no `std::fs` calls happen inside the WASM module

### openspec Plugin Tools

The openspec WASM plugin MUST expose these tools:

**spec_list**: List specs from a project.
- Input: `{"entries": [{"domain": "auth", "path": "...", "content": "..."}]}`
- Output: JSON array of `{"domain", "purpose", "requirement_count", "requirements"}`

**change_list**: List active changes.
- Input: `{"entries": [{"name": "...", "schema": "...", "tasks_content": "..."}]}`
- Output: JSON array of `{"name", "schema", "task_progress"}`

**change_verify**: Verify a change.
- Input: `{"tasks_content": "...", "has_specs_dir": bool}`
- Output: `{"items": [{"severity", "message"}], "summary": "..."}`

**artifact_status**: Show artifact DAG state.
- Input: `{"schema_artifacts": [...], "existing_files": [...]}`
- Output: `{"artifacts": [{"id", "state", "requires"}], "next_ready", "is_complete"}`

**spec_parse**: Parse a spec markdown file.
- Input: `{"content": "...", "domain": "auth"}`
- Output: `{"domain", "purpose", "requirements": [{"heading", "strength", "scenarios"}]}`

### openspec Plugin Manifest

The plugin MUST ship with a `plugin.json` manifest:

```json
{
  "name": "openspec",
  "version": "0.1.0",
  "description": "Spec-driven development tools — list, verify, and inspect OpenSpec artifacts",
  "wasm": "openspec_plugin.wasm",
  "kind": "extism",
  "permissions": [],
  "tools": ["spec_list", "spec_parse", "change_list", "change_verify", "artifact_status"],
  "events": ["agent_start"],
  "tool_definitions": [
    {
      "name": "spec_list",
      "description": "List project specs with domains, purposes, and requirement counts",
      "input_schema": {
        "type": "object",
        "properties": {
          "entries": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "domain": {"type": "string"},
                "path": {"type": "string"},
                "content": {"type": "string"}
              }
            }
          }
        },
        "required": ["entries"]
      }
    },
    {
      "name": "spec_parse",
      "description": "Parse a spec markdown file into structured requirements with GIVEN/WHEN/THEN scenarios",
      "input_schema": {
        "type": "object",
        "properties": {
          "content": {"type": "string", "description": "Markdown content of the spec file"},
          "domain": {"type": "string", "description": "Domain name (e.g. 'auth', 'protocol')"}
        },
        "required": ["content"]
      }
    },
    {
      "name": "change_list",
      "description": "List active OpenSpec changes with task progress summaries",
      "input_schema": {
        "type": "object",
        "properties": {
          "entries": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "name": {"type": "string"},
                "schema": {"type": "string"},
                "tasks_content": {"type": "string"}
              }
            }
          }
        },
        "required": ["entries"]
      }
    },
    {
      "name": "change_verify",
      "description": "Verify an OpenSpec change — check task completion and spec coverage",
      "input_schema": {
        "type": "object",
        "properties": {
          "tasks_content": {"type": "string"},
          "has_specs_dir": {"type": "boolean"}
        },
        "required": []
      }
    },
    {
      "name": "artifact_status",
      "description": "Show artifact dependency graph state for an OpenSpec change",
      "input_schema": {
        "type": "object",
        "properties": {
          "schema_artifacts": {
            "type": "array",
            "items": {
              "type": "object",
              "properties": {
                "id": {"type": "string"},
                "generates": {"type": "string"},
                "requires": {"type": "array", "items": {"type": "string"}}
              }
            }
          },
          "existing_files": {
            "type": "array",
            "items": {"type": "string"}
          }
        },
        "required": ["schema_artifacts"]
      }
    }
  ]
}
```
