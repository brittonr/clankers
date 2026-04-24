# crate-extraction-3 — Design

## Status

Draft. The remaining six extraction candidates still live in the clankers
workspace and have not been split into standalone repos yet.

## Goals / Non-Goals

**Goals**
- finish the remaining leaf/infrastructure extractions from the original
  second-pass plan
- preserve the runtime/feature contracts of each extracted crate
- keep workspace continuity after every extraction
- make final cleanup explicit, including generated artifact refresh

**Non-Goals**
- revisit already completed extractions from `crate-extraction-2`
- add WASM plugin wrappers for the remaining six crates
- redesign public APIs beyond what extraction and namespace cleanup require

## Decisions

### 1. Continue with leaves before infrastructure

**Choice:** Extract the remaining leaves first (`nix`, `matrix`, `zellij`), then
move to the infrastructure crates (`protocol`, `db`, `hooks`).

**Rationale:** The leaves have smaller blast radii and fewer reverse dependents.
They are the cheapest way to re-establish the extraction rhythm in the new
change before touching shared infrastructure.

### 2. nix keeps the existing snix pin and feature flags

**Choice:** Preserve the exact snix revision and the `eval` / `refscan`
feature flags in `clanker-nix`.

**Rationale:** The extracted crate should remain a drop-in replacement. snix is
still pre-1.0 and its pinned revision is part of the contract that already
works in the workspace.

### 3. matrix preserves the full matrix-sdk feature set

**Choice:** Keep `e2e-encryption`, `sqlite`, and `rustls-tls` enabled in the
extracted `clanker-matrix` crate.

**Rationale:** Stripping any of those features would silently degrade the
bridge's behavior and break the "drop-in extraction" goal.

### 4. zellij keeps iroh version alignment with the workspace

**Choice:** Pin `clanker-zellij` to the same iroh line currently used by the
workspace and preserve the `address-lookup-mdns` feature.

**Rationale:** iroh minor-version drift is risky in binaries that link several
related networking crates. Matching the workspace avoids split-world linking
and behavior drift.

### 5. protocol keeps tokio in the public implementation

**Choice:** Preserve the tokio-based framing layer instead of trying to rewrite
it around another async trait surface during extraction.

**Rationale:** The point of this change is extraction, not protocol redesign.
The existing `AsyncReadExt` / `AsyncWriteExt` framing is what clankers already
ships today.

### 6. db moves all table modules intact

**Choice:** Extract all existing redb tables together: audit, memory, sessions,
history, usage, file_cache, tool_results, and registry.

**Rationale:** The value of `clanker-db` is the whole storage layer, not just a
thin redb wrapper. Keeping the schemas together preserves the current API and
avoids a half-extracted crate.

### 7. hooks adds `Custom(String)` during extraction

**Choice:** Keep the concrete `HookPoint` enum but add `Custom(String)` as part
of the extracted crate.

**Rationale:** This was already the planned extensibility mechanism. The new
crate should not require another extraction cycle just to support non-clankers
hook points.

### 8. No WASM plugin work in this continuation change

**Choice:** Treat all six remaining crates as library-only extractions.

**Rationale:** Every remaining crate depends on native runtime features or
represents infrastructure/types that do not fit the plugin request/response
model.

### 9. Final cleanup must refresh generated workspace artifacts

**Choice:** Add explicit final-cleanup tasks for:
- `build-plan.json` regeneration via `unit2nix`
- generated docs refresh via `cargo xtask docs`
- snapshot refresh only if a rename changes user-visible TUI output

**Rationale:** Previous wrapper-removal work showed that these artifacts can
quietly drift even when the Rust workspace still compiles. The continuation
change should capture that repo-specific cleanup explicitly.

### 10. Shared dependency sources must be unified with `[patch]` entries when needed

**Choice:** Before the first extraction lands, audit the six crates for
dependencies on already-extracted or vendored crates. If an extracted crate's
published `Cargo.toml` points at a git source that the workspace vendors or
patches differently, add a root `[patch."<source-url>"]` entry so clankers uses
one source graph locally.

**Rationale:** `crate-extraction-2` hit this exact failure with
`clanker-router`: a git dep plus a vendored local snapshot produced two
incompatible type graphs. The continuation change should make the unification
policy explicit up front instead of rediscovering it mid-extraction.

### 11. Wrapper removal is per-crate, never a bulk surprise

**Choice:** Each extraction phase has three explicit sub-steps:
1. switch the workspace to the git dependency and keep a thin wrapper
2. migrate that crate's direct callers off `clankers_*` imports
3. remove that crate's wrapper only after its callers are migrated

**Rationale:** A single "remove all wrappers later" task hides the real seam and
makes it easy to claim verification before the migration is complete.
Infrastructure crates with more reverse dependents need per-crate migration
visibility.

### 12. protocol compatibility uses durable fixtures

**Choice:** Verify `clanker-protocol` with checked-in serde/framing fixtures for
`DaemonEvent`, `SessionCommand`, `ControlRequest`, and `ControlResponse`.

**Rationale:** Wire compatibility is a byte-level contract. A generic workspace
build cannot prove that serde output or framing stayed identical.

### 13. `HookPoint::Custom(String)` needs compatibility analysis and tests

**Choice:** Treat `Custom(String)` as the one behavioral addition in this
change. Before landing it:
- audit existing clankers matches for exhaustive `HookPoint` handling
- update those call sites as needed
- add serde round-trip and dispatcher tests for the new variant

**Rationale:** Adding a new public enum variant is a compatibility seam even if
all current clankers callers are updated in the same repo.

## Architecture

### Before continuation

The workspace still owns these extraction candidates directly:

```text
crates/clankers-nix/
crates/clankers-matrix/
crates/clankers-zellij/
crates/clankers-protocol/
crates/clankers-db/
crates/clankers-hooks/
```

### After continuation

```text
Standalone repos / git deps:
├── brittonr/clanker-nix/
├── brittonr/clanker-matrix/
├── brittonr/clanker-zellij/
├── brittonr/clanker-protocol/
├── brittonr/clanker-db/
└── brittonr/clanker-hooks/

clankers workspace:
- consumes those six crates via git deps
- keeps wrappers only while each crate's direct callers migrate
- removes wrappers one crate at a time after caller migration evidence exists
- regenerates `build-plan.json` and generated docs after the final wrapper cleanup
```

## Coupling Notes

### protocol
- reverse dependents: root crate, `clankers-controller`
- wire compatibility is the critical seam
- any shared extracted/vendored dependency must be unified with a root `[patch]`

### db
- reverse dependents: root crate, `clankers-agent`
- all table modules move together
- storage API changes are out of scope; extraction is structural only

### hooks
- reverse dependents: root crate, `clankers-agent`, `clankers-config`,
  `clankers-controller`, `clankers-plugin`
- `HookPoint::Custom(String)` requires caller audit plus dedicated tests

## Risks / Trade-offs

**Generated artifact drift after wrapper removal**
→ Mitigation: make `unit2nix`, docs refresh, and conditional snapshot refresh
explicit tasks, not implicit assumptions under a generic workspace-green task.

**Feature drift during extraction**
→ Mitigation: preserve the current pins/features in the design and spec deltas,
then verify them with extraction-specific tasks.

**Protocol/storage/hook regressions have wider blast radius than the leaves**
→ Mitigation: do the three leaf extractions first, keep full-workspace rails in
every phase, and use durable wire/custom-variant tests where generic builds are
not enough.

**Dirty sibling path dependencies can produce false negatives**
→ Mitigation: verify sibling path deps are clean before using a validation run
as evidence, or explicitly record external contamination.
