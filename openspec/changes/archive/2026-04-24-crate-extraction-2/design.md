# crate-extraction-2 — Design

## Status

Reduced-scope implementation is complete. `openspec` is consumed through the
checked-in `vendor/openspec` snapshot because the GitHub repo was not published
and this session did not have an explicit push request. The remaining six
extractions were split into `crate-extraction-3`.

## Goals / Non-Goals

**Goals**
- preserve the completed shared-crate extractions in one coherent change
- keep the `openspec` core/native split, vendored source pin, and plugin packaging documented here
- keep the two highest-fanout type migrations (`tui-types`, `message`) with
  their validation evidence
- leave the remaining six extractions to a separate continuation change

**Non-Goals**
- finish the remaining leaf/infrastructure extractions in this change
- redesign extracted crate APIs beyond what extraction required
- move filesystem access into the `openspec` WASM module

## Decisions

### 1. Split the original ten-crate plan into two changes

**Choice:** Keep the completed high-leverage work in `crate-extraction-2` and
move the remaining six extractions into `crate-extraction-3`.

**Rationale:** The shared SDK/spec/type work is already implemented and
verified. Leaving 63 unrelated todos in the same change obscures the real
completion boundary and makes archive readiness hard to judge.

**Alternative:** Keep all ten crates in one long-running change. Rejected
because it mixes finished migration work with untouched future work.

### 2. plugin-sdk stays a repo move, not a subtree split

**Choice:** Treat `clankers-plugin-sdk` as a clean standalone repo move and record this as a history-preservation exception.

**Rationale:** It already had its own `[workspace]`, targets wasm32, and is
consumed by external plugin crates rather than by the main clankers workspace.
A clean repo identity is simpler than preserving a mostly unrelated parent
workspace history slice.

### 2a. openspec is vendored until remote publication

**Choice:** Treat `openspec` as a curated extracted snapshot under `vendor/openspec` until an explicit remote publication/push happens.

**Rationale:** The local `~/git/openspec` repository contains the completed core/shell split and plugin work, but `brittonr/openspec` was not published. Vendoring keeps clankers reproducible without relying on a sibling checkout or requiring an implicit push. `vendor/openspec/VENDORED_FROM` records the source commit and snapshot status.

### 3. `openspec` uses a functional core + filesystem shell split

**Choice:** Keep `openspec` as two layers:
1. `openspec::core` — pure parsing, graph, delta, schema, and verification logic
2. root `openspec` — `SpecEngine`, config, and filesystem-backed helpers

**Rationale:** The pure core is testable without I/O and compiles for the WASM
plugin. The native shell preserves the ergonomic API used by clankers and the
standalone repo.

**Implementation:**
```text
openspec/
├── src/
│   ├── core/         # pure parsing / graph / verification logic
│   ├── engine.rs     # filesystem-backed SpecEngine
│   ├── config.rs     # filesystem-backed config helpers
│   └── lib.rs        # re-exports core + native API
└── openspec-plugin/  # wasm plugin crate
```

### 4. `openspec-plugin` remains a separate crate in the `openspec` repo

**Choice:** Keep the plugin in `openspec-plugin/` with
`openspec = { default-features = false }`.

**Rationale:** The plugin needs `clanker-plugin-sdk` / `extism-pdk`, but the
library crate should not inherit WASM-only runtime dependencies. A sibling
crate keeps the dependency graph clean and version coordination simple.

**Verification:**
- checked-in Extism integration test at `vendor/openspec/openspec-plugin/tests/runtime.rs`
- host-runtime smoke against the installed plugin under
  `~/.clankers/agent/plugins/openspec`

### 5. `clanker-tui-types` lands before `clanker-message`

**Choice:** Keep the high-fanout order as `tui-types` first, then `message`.

**Rationale:** `tui-types` had the widest blast radius and needed the
subwayrat dependency cleanup first. Once those callers were migrated, the
message extraction had fewer moving pieces and could focus on the router edge.

### 6. `clanker-message` shares the vendored router source graph

**Choice:** Keep the extracted `clanker-message` repo on a git dependency for
`clanker-router`, while patching the main workspace back to
`vendor/clanker-router`.

**Rationale:** This avoids compiling two distinct router source graphs inside
clankers and preserves shared `Usage` / stream-event types.

### 7. Final cleanup only removes wrappers for the reduced-scope crates

**Choice:** The final cleanup in this change covers only:
- `clankers-plugin-sdk`
- `clankers-specs`
- `clankers-tui-types`
- `clankers-message`

**Rationale:** The remaining six crates still live in-tree and now belong to
`crate-extraction-3`. Cleanup and grep assertions in this change must not
pretend those future extractions already happened.

## Architecture

### Before the reduced-scope extraction

The workspace still owned these shared crates directly:

```text
crates/clankers-plugin-sdk/
crates/clankers-specs/
crates/clankers-tui-types/
crates/clankers-message/
```

### After the reduced-scope extraction

```text
Standalone repos / checked-in source deps:
├── brittonr/clanker-plugin-sdk/
├── vendor/openspec/                 # vendored snapshot until remote publication
│   └── openspec-plugin/
├── brittonr/clanker-tui-types/
└── brittonr/clanker-message/

clankers workspace:
- consumes plugin-sdk, tui-types, and message via git deps
- consumes openspec via the checked-in vendored path dependency
- no longer keeps wrapper crates for those four extracted crates
- keeps the remaining six planned extractions in-tree until crate-extraction-3
```

### openspec plugin data flow

```text
LLM invokes tool
  → host gathers filesystem data / file contents
  → host serializes JSON args
  → openspec-plugin calls pure openspec::core helpers
  → plugin returns structured JSON
  → host shows the result to the agent
```

The WASM module stays pure. Filesystem reads remain in the host.

## Risks / Trade-offs

**Remote publishing lag for `openspec`**
→ Mitigation: vendor the extracted snapshot under `vendor/openspec` and record
its source commit in `vendor/openspec/VENDORED_FROM` so fresh checkouts and Nix
builds no longer depend on a sibling `../openspec` path. Remote publication can
happen later without blocking this change.

**Evidence drift between checked-in tests and ad-hoc smoke scripts**
→ Mitigation: lead with checked-in test evidence (`openspec-plugin/tests/runtime.rs`)
and treat `/tmp` smoke scripts as supplemental runtime confirmation only.

**Future extraction work could accidentally regress the completed slice**
→ Mitigation: `crate-extraction-3` gets its own tasks/specs and will rerun the
workspace validation bundle after each extraction and at final cleanup.
