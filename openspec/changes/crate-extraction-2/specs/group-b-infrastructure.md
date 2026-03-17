# Group B: Infrastructure Extractions — Spec

## Purpose

Contracts for four infrastructure crates with zero internal dependencies and
moderate reverse dependency counts. These require more care because multiple
workspace crates import them.

## Requirements

### protocol Extraction

The `clankers-protocol` crate MUST be extracted to `clanker-protocol`.
The framing layer (4-byte length prefix + JSON) and all command/event/control
types MUST move together.

GIVEN `crates/clankers-protocol/` with modules: command, control, event, frame, types
WHEN extracted to `clanker-protocol` repo
THEN `read_frame` and `write_frame` async functions work with any `AsyncRead`/`AsyncWrite`
AND `DaemonEvent`, `SessionCommand`, `ControlRequest`, `ControlResponse` serialize/
    deserialize identically to the pre-extraction wire format
AND the tokio dependency is preserved (framing uses `AsyncReadExt`/`AsyncWriteExt`)
AND 2 reverse dependents (root crate, clankers-controller) compile via re-export wrapper

### specs Extraction

The `clankers-specs` crate MUST be extracted to `openspec`. The entire
spec engine moves: schema, artifacts, changes, specs, delta, merge, verify,
templates, and config modules.

GIVEN `crates/clankers-specs/` with petgraph + serde_yaml deps
WHEN extracted to `openspec` repo
THEN `SpecEngine::new()`, `init()`, `discover_specs()`, `discover_changes()`,
    `create_change()`, `archive_change()`, `sync_change()`, `verify_change()`,
    and `specs_for_context()` all work
AND the `Schema`, `Artifact`, `ArtifactGraph`, `Spec`, `Requirement`, `Scenario`,
    `ChangeInfo`, `TaskProgress`, `DeltaSpec`, `SyncResult`, `VerifyReport` types
    are all public
AND the builtin `spec-driven` schema is available via `builtin_spec_driven()`
AND all `clankers_specs` references are renamed to `openspec`

### specs WASM Plugin

In addition to the library extraction, an `openspec-plugin` crate MUST be
created that wraps the pure parsing/graph logic as a clankers WASM plugin.

GIVEN the `openspec` library crate
WHEN an `openspec-plugin` crate is built targeting wasm32-unknown-unknown
THEN it exposes these tools via the plugin SDK:
  - `spec_list` — list discovered specs with domains and requirement counts
  - `spec_search` — find specs matching a keyword or domain
  - `change_list` — list active changes with task progress
  - `change_verify` — run verification on a change, return report
  - `artifact_status` — show artifact graph state for a change
AND it handles `agent_start` events to log readiness
AND filesystem paths are passed via tool args (the host resolves them)
AND the plugin compiles with `cargo build --target wasm32-unknown-unknown`

The plugin MUST separate pure logic (parsing, graph traversal, verification)
from filesystem access. The tool handlers receive file contents or directory
listings as string arguments rather than reading the filesystem directly.

GIVEN a `spec_list` tool call with `{"specs_dir": "/path/to/specs"}`
WHEN the host invokes the plugin
THEN the plugin returns a JSON array of spec summaries
AND no `std::fs` calls happen inside the WASM module

### db Extraction

The `clankers-db` crate MUST be extracted to `clanker-db`. All table
modules (audit, memory, sessions, history, usage, file_cache, tool_results,
registry) MUST move. The redb dependency MUST be preserved.

GIVEN `crates/clankers-db/` with redb 2.6 and modules for 8 table types
WHEN extracted to `clanker-db` repo
THEN all table definitions and their read/write methods work
AND the `schema.rs` module correctly defines all redb table types
AND the `error.rs` module provides typed errors
AND 2 reverse dependents (root crate, clankers-agent) compile via re-export wrapper

### hooks Extraction

The `clankers-hooks` crate MUST be extracted to `clanker-hooks`. The
dispatch pipeline, hook points, verdicts, and script execution MUST all move.

GIVEN `crates/clankers-hooks/` with modules: config, dispatcher, git, payload, point,
      script, verdict
WHEN extracted to `clanker-hooks` repo
THEN `HookPipeline`, `HookHandler`, `HookVerdict`, `HookPoint`, `HookPayload`,
    `HookConfig`, and `GitHooks` types are public
AND the async `HookHandler` trait works with tokio
AND script hook execution (subprocess spawning) works
AND 5 reverse dependents (root, agent, config, controller, plugin) compile
    via re-export wrapper
