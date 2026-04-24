# crate-extraction-2 — Design

## Status

Not started. All 10 crates still live in the workspace.

## Decisions

### Three groups, ordered by risk

**Choice:** Group A (leaves) → Group B (infrastructure) → Group C (types)
**Rationale:** Leaves have 1–2 reverse deps — if something breaks, the
blast radius is small. Infrastructure crates have 2–5 reverse deps and
need re-export wrappers. Type crates have 10 reverse deps each and need
the most careful migration. Building confidence on easy extractions before
tackling the high-fanout ones.

### plugin-sdk: repo move, not subtree split

**Choice:** Copy the directory into a new repo rather than `git subtree split`.
**Rationale:** `clankers-plugin-sdk` already declares its own `[workspace]`
and isn't part of the main workspace's dependency graph. It's consumed by
external plugin crates, not by the workspace itself. A clean repo start is
simpler than splitting a directory that was never a workspace member.
**Alternative:** subtree split. Works but produces a confusing history since
the crate's Cargo.toml has `[workspace]` overriding the parent.

### nix: keep snix rev pinned

**Choice:** Carry the exact `rev = "8fe3bade..."` pin into the extracted repo.
**Rationale:** snix is pre-1.0 and evolving. The pin guarantees the extracted
crate compiles with the same snix version. Upgrading snix can happen in the
extracted repo independently of clankers.

### matrix: preserve full feature set

**Choice:** Keep `e2e-encryption`, `sqlite`, `rustls-tls` features on
matrix-sdk in the extracted crate.
**Rationale:** Dropping any of these would break E2E encryption or
persistence. The extracted crate should be a drop-in replacement.

### zellij: iroh version alignment

**Choice:** Pin iroh to the same version used in the clankers workspace.
**Rationale:** iroh has breaking changes between minor versions. The
workspace's iroh version is used by the daemon's QUIC bridge too. Version
mismatch could cause link errors if both are in the same binary.

### protocol: tokio stays

**Choice:** Keep tokio as a dependency in `clanker-protocol`.
**Rationale:** `read_frame`/`write_frame` use `AsyncReadExt`/`AsyncWriteExt`.
Making the framing generic over `futures::AsyncRead` would require an
API-breaking refactor. Not worth it for the extraction — do it later if
someone needs a non-tokio runtime.

### specs → openspec: pure core + filesystem shell

**Choice:** Restructure `openspec` into two layers:
1. `openspec::core` — pure functions that take strings/structs and return
   structs. No `std::fs`. Compilable to wasm32.
2. `openspec` (root) — `SpecEngine` and convenience functions that use
   `std::fs`. Not wasm-compatible. Re-exports core types.

**Rationale:** The WASM plugin needs the parsing and graph logic but can't
do filesystem I/O. Separating them lets the plugin depend on `openspec::core`
while the native library keeps the filesystem convenience API.

**Implementation:**
```
openspec/
├── src/
│   ├── lib.rs          # SpecEngine (filesystem), re-exports core::*
│   ├── core/
│   │   ├── mod.rs      # pub mod spec, artifact, change, delta, ...
│   │   ├── spec.rs     # parse_spec_content(content: &str) -> Spec
│   │   ├── artifact.rs # ArtifactGraph::from_state(artifacts, existing_files)
│   │   ├── change.rs   # TaskProgress parsing from string
│   │   ├── delta.rs    # parse_delta_content(content: &str) -> DeltaSpec
│   │   ├── merge.rs    # merge operations on in-memory data
│   │   ├── schema.rs   # Schema, SchemaArtifact types
│   │   ├── verify.rs   # verify_from_content(tasks: &str, has_specs: bool)
│   │   └── templates.rs
│   ├── engine.rs       # SpecEngine (uses std::fs, calls core::*)
│   └── config.rs       # SpecConfig (uses std::fs)
└── Cargo.toml          # features: default=["fs"], fs=[]
```

The `fs` feature gates `SpecEngine` and `config.rs`. Without it, only
`core` types are available — wasm-compatible.

### specs WASM plugin: separate crate

**Choice:** `openspec-plugin` is a separate crate in the same repo (or a
sibling repo) that depends on `openspec` with `default-features = false`.

**Rationale:** The plugin needs `clankers-plugin-sdk` (extism-pdk) which
targets wasm32. Keeping it in the same repo as the library makes version
coordination easy. Keeping it in a separate crate avoids polluting the
library's dependency tree with extism-pdk.

```
openspec/
├── Cargo.toml          # library crate
├── src/
└── openspec-plugin/
    ├── Cargo.toml      # [lib] crate-type = ["cdylib"], deps: openspec (no fs), clankers-plugin-sdk
    ├── src/lib.rs      # tool handlers, event handlers, describe()
    └── plugin.json     # manifest
```

### db: keep all table schemas

**Choice:** Move all 8 table modules as-is.
**Rationale:** The tables are generic key-value patterns over redb. The
column names are descriptive ("audit_log", "memory_store") but not
tightly coupled to clankers internals. Other projects could use the same
table schemas or define their own using the same patterns.
**Alternative:** Only extract the redb wrapper/error types and leave
table definitions in-tree. More work for marginal benefit — the table
modules are already well-isolated.

### hooks: HookPoint stays concrete

**Choice:** Extract `clanker-hooks` with the concrete `HookPoint` enum
(PreCommit, SessionStart, ToolPre, ToolPost, etc.).
**Rationale:** Generalizing HookPoint into a trait was considered in phase 1
and rejected. The enum is small (8 variants) and other projects that use
this crate can either use the existing variants or we add a `Custom(String)`
variant for extensibility.
**Change from phase 1:** Phase 1 decided not to extract hooks. Re-evaluating
because the hook system has matured and 5 crates depend on it — extracting
it reduces workspace coupling significantly.

### tui-types: resolve subwayrat path deps

**Choice:** Convert `rat-branches` and `rat-leaderkey` path deps to git
deps pointing at the subwayrat repository before extracting tui-types.
**Rationale:** The extracted `clanker-tui-types` can't have path deps to
a sibling repo. Git deps work for the same reason they work for the
already-extracted crates.

### message: extract after tui-types

**Choice:** Extract `clanker-message` after `clanker-tui-types`.
**Rationale:** `clankers-message` depends on `clanker-router` (done) but
several of its reverse dependents also depend on `clankers-tui-types`. If
tui-types is extracted first, the message extraction has fewer moving parts.

### New crate names

| Workspace crate | Extracted name | Rationale |
|---|---|---|
| clankers-plugin-sdk | `clanker-plugin-sdk` | Drop the s. Consistent with clanker-* convention. |
| clankers-nix | `clanker-nix` | Drop the s. |
| clankers-matrix | `clanker-matrix` | Drop the s. |
| clankers-zellij | `clanker-zellij` | Drop the s. |
| clankers-protocol | `clanker-protocol` | Drop the s. |
| clankers-specs | `openspec` | Standalone identity. The spec engine isn't clankers-specific. |
| clankers-db | `clanker-db` | Drop the s. |
| clankers-hooks | `clanker-hooks` | Drop the s. |
| clankers-tui-types | `clanker-tui-types` | Drop the s. |
| clankers-message | `clanker-message` | Drop the s. |

## Architecture

### Before extraction

```
clankers (workspace) — 24 crates
├── crates/clankers-plugin-sdk/  (own [workspace], 0 reverse deps in main workspace)
├── crates/clankers-nix/         (0 internal deps, 1 reverse dep)
├── crates/clankers-matrix/      (0 internal deps, 1 reverse dep)
├── crates/clankers-zellij/      (0 internal deps, 1 reverse dep)
├── crates/clankers-protocol/    (0 internal deps, 2 reverse deps)
├── crates/clankers-specs/       (0 internal deps, 2 reverse deps)
├── crates/clankers-db/          (0 internal deps, 2 reverse deps)
├── crates/clankers-hooks/       (0 internal deps, 5 reverse deps)
├── crates/clankers-tui-types/   (0 internal deps, 10 reverse deps)
├── crates/clankers-message/     (1 internal dep, 7 reverse deps)
└── ... 14 other crates
```

### After extraction

```
GitHub repos (git deps):
├── brittonr/clanker-plugin-sdk/   (from clankers-plugin-sdk)
├── brittonr/clanker-nix/          (from clankers-nix)
├── brittonr/clanker-matrix/       (from clankers-matrix)
├── brittonr/clanker-zellij/       (from clankers-zellij)
├── brittonr/clanker-protocol/     (from clankers-protocol)
├── brittonr/openspec/             (from clankers-specs, + openspec-plugin)
├── brittonr/clanker-db/           (from clankers-db)
├── brittonr/clanker-hooks/        (from clankers-hooks)
├── brittonr/clanker-tui-types/    (from clankers-tui-types)
└── brittonr/clanker-message/      (from clankers-message)

clankers (workspace) — 14 crates remaining
├── crates/clankers-agent/
├── crates/clankers-agent-defs/
├── crates/clankers-config/
├── crates/clankers-controller/
├── crates/clankers-model-selection/
├── crates/clankers-plugin/
├── crates/clankers-procmon/
├── crates/clankers-prompts/
├── crates/clankers-provider/
├── crates/clankers-session/
├── crates/clankers-skills/
├── crates/clankers-tui/
├── crates/clankers-ucan/
├── crates/clankers-util/
└── xtask/
```

### openspec Plugin Data Flow

```
LLM invokes "spec_list" tool
  → clankers host reads openspec/specs/ directory
  → host builds JSON: {"entries": [{"domain":"auth","content":"# Auth spec\n..."}]}
  → host calls plugin WASM: handle_tool_call(json)
    → plugin deserializes entries
    → plugin calls openspec::core::spec::parse_spec_content() for each
    → plugin builds response JSON
  → host returns tool result to LLM
```

The host handles all filesystem access. The plugin is a pure
parse-and-transform layer.

## Coupling Analysis

### clankers-tui-types (10 reverse deps) — highest impact

```
Cargo.toml (root)                      — direct dep
crates/clankers-agent/Cargo.toml       — direct dep
crates/clankers-config/Cargo.toml      — direct dep
crates/clankers-controller/Cargo.toml  — direct dep
crates/clankers-model-selection/       — direct dep
crates/clankers-plugin/Cargo.toml      — direct dep
crates/clankers-procmon/Cargo.toml     — direct dep
crates/clankers-provider/Cargo.toml    — direct dep
crates/clankers-tui/Cargo.toml         — direct dep
crates/clankers-util/Cargo.toml        — direct dep
```

Every crate that interacts with the UI imports tui-types. The re-export
wrapper is load-bearing. Don't remove it until all 10 are migrated.

### clankers-message (7 reverse deps)

```
Cargo.toml (root)
crates/clankers-agent/Cargo.toml
crates/clankers-controller/Cargo.toml
crates/clankers-engine/Cargo.toml
crates/clankers-provider/Cargo.toml
crates/clankers-session/Cargo.toml
crates/clankers-util/Cargo.toml
```

Core message types. All callers use `Message`, `Role`, `Content`.
Re-export wrapper needed during migration.

### clankers-hooks (5 reverse deps)

```
Cargo.toml (root)
crates/clankers-agent/Cargo.toml
crates/clankers-config/Cargo.toml
crates/clankers-controller/Cargo.toml
crates/clankers-plugin/Cargo.toml
```

Hook points and dispatch. The `HookHandler` trait is implemented by
the agent and plugin systems.
