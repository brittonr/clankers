# crate-extraction-3

## Intent

`crate-extraction-2` landed the highest-leverage shared extractions and was
split so the remaining untouched work can continue in a focused change.

`crate-extraction-3` continues the second extraction pass for the six crates
that still live in the clankers workspace:

1. **clankers-nix** → `clanker-nix`
2. **clankers-matrix** → `clanker-matrix`
3. **clankers-zellij** → `clanker-zellij`
4. **clankers-protocol** → `clanker-protocol`
5. **clankers-db** → `clanker-db`
6. **clankers-hooks** → `clanker-hooks`

These are now the remaining standalone leaf/infrastructure extractions from the
original ten-crate pass.

## Scope

### In Scope

#### Group A — remaining leaves
- **clankers-nix** → `clanker-nix`
  - preserve the snix pin and feature flags
- **clankers-matrix** → `clanker-matrix`
  - preserve the matrix-sdk feature set
- **clankers-zellij** → `clanker-zellij`
  - preserve iroh version alignment and mDNS discovery support

#### Group B — remaining infrastructure
- **clankers-protocol** → `clanker-protocol`
  - preserve framing and daemon/client wire compatibility
- **clankers-db** → `clanker-db`
  - move all redb table modules intact
- **clankers-hooks** → `clanker-hooks`
  - preserve hook dispatch/runtime behavior while adding the planned
    `Custom(String)` extension point

#### Verification and cleanup
- migrate each extracted crate into a git dependency
- use re-export wrappers only while callers migrate
- remove wrappers once direct callers are updated
- refresh generated workspace artifacts after wrapper removal
- verify the full workspace after each extraction and at final cleanup

### Out of Scope

Already completed in `crate-extraction-2`:
- `clankers-plugin-sdk` → `clanker-plugin-sdk`
- `clankers-specs` → `openspec`
- `openspec-plugin`
- `clankers-tui-types` → `clanker-tui-types`
- `clankers-message` → `clanker-message`

Still out of scope for this extraction pass:
- `clankers-agent-defs`
- `clankers-prompts`
- `clankers-skills`
- `clankers-procmon`
- `clankers-model-selection`
- `clankers-provider`
- core app/runtime crates (`agent`, `controller`, `config`, `tui`, `session`, `plugin`, `util`)

## WASM Plugin Assessment

None of the remaining six crates qualify for WASM plugin packaging.

| Crate | Blocking reason |
|---|---|
| clanker-nix | snix / native Nix store interaction |
| clanker-matrix | matrix-sdk with sqlite/TLS/crypto |
| clanker-zellij | iroh QUIC + tokio runtime |
| clanker-protocol | tokio framing layer |
| clanker-db | redb mmap / file locks |
| clanker-hooks | tokio + subprocess/script dispatch |

This change extracts them as library crates only.

## Verification Expectations

Completion evidence for this change includes:
- per-phase workspace `cargo check && cargo nextest run`
- protocol wire-format fixture coverage for `clanker-protocol`
- explicit verification for `HookPoint::Custom(String)`
- generated artifact refresh (`build-plan.json`, generated docs, and snapshots
  only if a rename touches user-visible TUI output)
- a final full-workspace `RUSTC_WRAPPER= cargo check && RUSTC_WRAPPER= cargo nextest run`

## Approach

Reuse the proven extraction protocol from the earlier crates:

1. create/publish the standalone repo
2. preserve history with `git subtree split`
3. rename into the standalone namespace
4. add standalone repo scaffolding (README, LICENSE, CI)
5. switch the workspace to a git dependency
6. keep a thin wrapper only while callers are migrating
7. remove wrappers once callers use the extracted crate directly
8. regenerate generated workspace artifacts after wrapper removal
9. rerun full-workspace validation after each risky extraction and at final cleanup

Execution order is fixed and sequential:
- Phase 1 `nix`
- Phase 2 `matrix`
- Phase 3 `zellij`
- Phase 4 `protocol`
- Phase 5 `db`
- Phase 6 `hooks`

No phases are interleaved and no bulk migration is allowed.
