# crate-extraction — Design

## Status

Not started. All six crates still live in the workspace.

## Decisions

### Extraction order: leaf crates first, high-value crates early

**Choice:** merge -> actor -> scheduler -> loop -> router -> auth
**Rationale:** merge and actor have zero workspace dependencies and zero
clankers-specific code in their source files. They can be extracted by
renaming and pushing to GitHub — no refactoring needed. Scheduler and loop are
similarly clean. Router is the highest-value extraction (16k lines, own
binary, useful to anyone building LLM tools) but larger. Auth needs its
Capability enum generalized before it's reusable.

### Re-export from original location during migration

**Choice:** After extraction, `crates/clankers-merge/` becomes a thin
wrapper that re-exports from the extracted `graggle` crate (pulled via
git dep). Same for each extracted crate.
**Rationale:** Internal code keeps compiling without a mass find-replace
of import paths. The thin wrapper can be removed later when convenient.
**Alternative considered:** Immediate find-replace of all imports. Risks
breaking things across 14+ crates and the main binary in one commit.

### New crate names

| Workspace crate | Extracted name | Rationale |
|---|---|---|
| clankers-merge | `graggle` | The algorithm's name (graph-file). Short, memorable. |
| clankers-actor | `erlactor` | Erlang-style actors. Clear, not taken. |
| clankers-scheduler | `cron-tick` | Cron-like tick engine. Descriptive. |
| clankers-loop | `iter-engine` | Iteration engine. Generic enough. |
| clankers-router | `llm-router` | What it does. The existing binary already works standalone. |
| clankers-auth | `ucan-cap` | UCAN capability tokens. Ties to the spec it implements. |

Names are placeholders — check GitHub availability before creating repos.

### Router keeps its binary

**Choice:** `llm-router` ships both a library crate and the `llm-router`
binary (renamed from `clankers-router`).
**Rationale:** The router already has its own `main.rs` with a TUI, proxy
server, and iroh tunnel. It should work as a standalone tool.

### Auth generalization strategy

**Choice:** Make `Capability` and `Operation` generic via a trait bound,
keep clankers-specific variants in a separate module or in the clankers
workspace.
**Rationale:** The token/builder/verifier machinery (~600 lines) is
generic — it signs, verifies, and delegates over any `Capability` type.
The specific capability variants (Prompt, ToolUse, FileAccess, etc.) are
clankers-domain. Splitting them lets other projects define their own
capability types and reuse the token infrastructure.
**Alternative considered:** Keep `Capability` as a concrete enum with
clankers variants. Then nobody else can use the crate without depending
on clankers-domain types. Defeats the purpose.

### Loop crate truncation module stays

**Choice:** `truncation.rs` (output truncation + temp file overflow)
moves with the loop crate.
**Rationale:** The truncation logic is generic — truncate large text to
N lines or N bytes, save overflow to a temp file, return a note about
where the full output went. One doc comment mentions "clankers" — change
it to reference the crate name instead. The module has no workspace deps.

### Protocol framing is NOT extracted

**Choice:** `read_frame` / `write_frame` (4-byte length prefix + JSON)
stays in `clankers-protocol`.
**Rationale:** It's ~50 lines of code. The pattern is trivial enough that
anyone who needs length-prefixed framing will write their own or use
tokio-util's `LengthDelimitedCodec`. Extracting 50 lines into a crate
creates more maintenance overhead than value.

### Hooks crate is NOT extracted

**Choice:** `clankers-hooks` stays in the workspace.
**Rationale:** The dispatch pipeline (`HookPipeline`, `HookHandler` trait,
`HookVerdict`) is generic, but the `HookPoint` enum (PreCommit, SessionStart,
ToolPre, ToolPost, etc.) is domain-specific. Generalizing HookPoint into a
trait adds complexity for a crate that's only ~300 lines of dispatch logic.
If another project needs lifecycle hooks, reconsider.

## Architecture

### Before extraction

```
clankers (workspace)
├── crates/clankers-merge/     (0 workspace deps)
├── crates/clankers-actor/     (0 workspace deps)
├── crates/clankers-scheduler/ (0 workspace deps)
├── crates/clankers-loop/      (0 workspace deps)
├── crates/clankers-router/    (0 workspace deps, 16k lines, own binary)
├── crates/clankers-auth/      (0 workspace deps, needs Capability generalization)
├── crates/clankers-agent/     (depends on merge, actor, loop, router, ...)
├── crates/clankers-controller/(depends on actor, loop, protocol, ...)
└── ... 20+ other crates
```

### After extraction

```
GitHub repos (git deps):
├── brittonr/graggle/       (from clankers-merge)
├── brittonr/erlactor/       (from clankers-actor)
├── brittonr/cron-tick/      (from clankers-scheduler)
├── brittonr/iter-engine/    (from clankers-loop)
├── brittonr/llm-router/     (from clankers-router)
└── brittonr/ucan-cap/       (from clankers-auth)

clankers (workspace):
├── crates/clankers-merge/      -> re-exports graggle (git dep)
├── crates/clankers-actor/      -> re-exports erlactor (git dep)
├── crates/clankers-scheduler/  -> re-exports cron-tick (git dep)
├── crates/clankers-loop/       -> re-exports iter-engine (git dep)
├── crates/clankers-router/     -> re-exports llm-router (git dep)
├── crates/clankers-auth/       -> depends on ucan-cap (git dep), adds clankers Capability impl
├── crates/clankers-agent/      (unchanged imports, gets new crates transitively)
└── ... everything else unchanged
```

### Migration path per crate

```
1. git subtree split -P crates/clankers-foo -b extract-foo
2. Create GitHub repo, push the branch
3. Rename crate in Cargo.toml, update module paths
4. Strip clankers references from docs/comments
5. Add CI (cargo test, clippy, fmt, nextest)
6. In clankers workspace:
   - crates/clankers-foo/Cargo.toml: add git dep on new repo
   - crates/clankers-foo/src/lib.rs: `pub use new_crate::*;`
   - Remove moved source files
   - cargo check, cargo nextest run
7. Later: grep for clankers_foo imports, replace with new_crate directly
8. Eventually: remove the thin wrapper crate from workspace
```

## Coupling Analysis

### clankers-merge (2 call sites)

```
crates/clankers-merge/src/lib.rs    — doc comment mentions "clankers_merge"
verus/merge_spec.rs                 — Verus proof references merge types
src/worktree/merge_strategy.rs      — uses Graggle, merge()
```

Two callers. Verus proof references types by name — update the import path
or keep the re-export wrapper.

### clankers-actor (14 call sites)

```
src/modes/daemon/agent_process.rs   — ProcessRegistry, spawn, link
src/modes/daemon/socket_bridge.rs   — ProcessRegistry, named processes
src/modes/daemon/quic_bridge.rs     — process lifecycle
src/modes/daemon/mod.rs             — registry init
src/modes/daemon/handlers.rs        — process signals
src/modes/matrix_bridge/*.rs        — 4 files use actor spawning
src/tools/subagent.rs               — spawn subagent actors
tests/socket_bridge.rs              — integration tests
crates/clankers-actor/tests/        — unit tests (move with crate)
verus/actor_spec.rs                 — Verus proof
```

Heaviest usage of any extraction candidate. All callers use
`ProcessRegistry`, `ProcessId`, `Signal`, `DeathReason`. The re-export
wrapper is load-bearing here — don't remove it until all callers migrate.

### clankers-router (26 files)

```
crates/clankers-provider/           — re-exports registry, retry, Model, Usage
crates/clankers-message/            — re-exports Usage
crates/clankers-model-selection/    — uses Model, routing policy
crates/clankers-agent/              — uses provider types transitively
src/                                — CLI, tools, slash commands
```

Most callers go through `clankers-provider`, which re-exports router types.
The provider crate becomes the main integration point. Router's own binary
moves to the new repo.

### clankers-auth (7 call sites)

```
crates/clankers-auth/src/           — the crate itself (move)
crates/clankers-util/src/           — path_policy uses auth types
src/commands/token.rs               — CLI token management
src/modes/matrix_bridge/            — bot command token handling
src/modes/daemon/                   — auth layer, session store
```

Moderate coupling. The Capability enum is referenced in daemon and matrix
bridge code. After extraction, `clankers-auth` becomes a thin crate that
defines `ClankerCapability` implementing `ucan-cap`'s generic capability
trait, plus the daemon auth layer.

### clankers-scheduler (1 call site)

```
src/tools/schedule.rs               — schedule tool
```

One caller. Easiest extraction after merge.

### clankers-loop (7 call sites)

```
crates/clankers-controller/         — loop mode, auto test
crates/clankers-agent/              — agent loop, turn execution
src/tools/loop_tool.rs              — loop tool
src/main.rs                         — cleanup temp files
```

Moderate coupling but all callers use the same surface: `LoopEngine`,
`LoopDef`, `BreakCondition`, `truncate_tool_output`. Clean API boundary.
