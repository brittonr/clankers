# crate-extraction-3 — Tasks

> **Legend:** `[ ]` not started · `[~]` in progress ⏱ · `[x]` done ✅ `<duration>`
>
> **Status:** Split out of `crate-extraction-2` on 2026-04-24. This change
> owns the remaining nix / matrix / zellij / protocol / db / hooks extractions.

## Phase 0: Shared preflight

- [x] Audit the six extraction targets for dependencies on already-extracted or vendored crates; add any required root `[patch."<source-url>"]` entries before the first migration lands (`evidence/preflight-audit.md`: no new patches needed)
- [x] Verify sibling path dependencies used by validation rails are clean (or record external contamination before treating failures as extraction regressions) (`evidence/preflight-audit.md`: `../subwayrat`, `../ratcore`, and `../openspec` dirty; treat as external contamination)
- [x] Decide whether any planned rename can affect user-visible TUI snapshots; if so, include snapshot refresh in final cleanup evidence (`evidence/preflight-audit.md`: no expected snapshot impact; keep final refresh conditional)

## Phase 1: nix (clankers-nix → clanker-nix)

Leaf extraction. Zero internal deps. snix git deps carry over.

- [ ] Create `clanker-nix` repo on GitHub
- [ ] `git subtree split -P crates/clankers-nix -b extract-nix`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-nix"`)
- [ ] Replace all `clankers_nix` / `clankers-nix` references in source
- [ ] Preserve feature flags: `eval`, `refscan`
- [ ] Verify the extracted repo still pins snix rev `8fe3bade...` and exposes `eval` / `refscan`
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate remaining callers to `clanker_nix`
- [ ] Remove the `clankers-nix` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 2: matrix (clankers-matrix → clanker-matrix)

Leaf extraction. Zero internal deps. Heavy external deps (`matrix-sdk`).

- [ ] Create `clanker-matrix` repo on GitHub
- [ ] `git subtree split -P crates/clankers-matrix -b extract-matrix`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-matrix"`)
- [ ] Replace all `clankers_matrix` / `clankers-matrix` references in source
- [ ] Preserve matrix-sdk features: `e2e-encryption`, `sqlite`, `rustls-tls`
- [ ] Verify the extracted repo still enables the full matrix-sdk feature set
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate remaining callers to `clanker_matrix`
- [ ] Remove the `clankers-matrix` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 3: zellij (clankers-zellij → clanker-zellij)

Leaf extraction. Zero internal deps. iroh QUIC dep.

- [ ] Create `clanker-zellij` repo on GitHub
- [ ] `git subtree split -P crates/clankers-zellij -b extract-zellij`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-zellij"`)
- [ ] Replace all `clankers_zellij` / `clankers-zellij` references in source
- [ ] Preserve iroh version and `address-lookup-mdns` feature
- [ ] Verify the extracted repo keeps iroh version alignment and the mDNS feature
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate remaining callers to `clanker_zellij`
- [ ] Remove the `clankers-zellij` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 4: protocol (clankers-protocol → clanker-protocol)

Infrastructure extraction. Zero internal deps. 2 reverse deps.

- [ ] Create `clanker-protocol` repo on GitHub
- [ ] `git subtree split -P crates/clankers-protocol -b extract-protocol`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-protocol"`)
- [ ] Replace all `clankers_protocol` / `clankers-protocol` references in source
- [ ] Verify frame, command, control, event, and types modules all compile
- [ ] Add checked-in wire fixtures for `DaemonEvent`, `SessionCommand`, `ControlRequest`, and `ControlResponse`
- [ ] Verify framing + serde round-trips preserve the pre-extraction wire contract
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate root crate imports to `clanker_protocol`
- [ ] Migrate `clankers-controller` imports to `clanker_protocol`
- [ ] Remove the `clankers-protocol` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on root crate + controller)

## Phase 5: db (clankers-db → clanker-db)

Infrastructure extraction. Zero internal deps. 2 reverse deps.

- [ ] Create `clanker-db` repo on GitHub
- [ ] `git subtree split -P crates/clankers-db -b extract-db`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-db"`)
- [ ] Replace all `clankers_db` / `clankers-db` references in source
- [ ] Verify all 8 table modules compile: audit, memory, sessions, history,
      usage, file_cache, tool_results, registry
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate root crate imports to `clanker_db`
- [ ] Migrate `clankers-agent` imports to `clanker_db`
- [ ] Remove the `clankers-db` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on root crate + agent)

## Phase 6: hooks (clankers-hooks → clanker-hooks)

Infrastructure extraction. Zero internal deps. 5 reverse deps.

- [ ] Create `clanker-hooks` repo on GitHub
- [ ] `git subtree split -P crates/clankers-hooks -b extract-hooks`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-hooks"`)
- [ ] Replace all `clankers_hooks` / `clankers-hooks` references in source
- [ ] Add `Custom(String)` variant to `HookPoint` for extensibility
- [ ] Verify config, dispatcher, git, payload, point, script, and verdict modules compile
- [ ] Add dedicated tests for `HookPoint::Custom(String)` serde round-trip and dispatcher behavior
- [ ] Audit existing clankers `HookPoint` matches and update exhaustive callers for the new variant
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] Migrate root crate imports to `clanker_hooks`
- [ ] Migrate `clankers-agent` imports to `clanker_hooks`
- [ ] Migrate `clankers-config` imports to `clanker_hooks`
- [ ] Migrate `clankers-controller` imports to `clanker_hooks`
- [ ] Migrate `clankers-plugin` imports to `clanker_hooks`
- [ ] Remove the `clankers-hooks` thin wrapper crate
- [ ] `cargo check && cargo nextest run` on full workspace (with emphasis on all 5 reverse deps)

## Phase 7: Final cleanup

- [ ] Grep workspace for any remaining `clankers_nix`, `clankers_matrix`, `clankers_zellij`, `clankers_protocol`, `clankers_db`, `clankers_hooks`
- [ ] Confirm no thin wrapper crates remain for the continuation extractions
- [ ] Verify `Cargo.lock` records the six git-dependency migrations cleanly
- [ ] Update workspace `members` list in root Cargo.toml
- [ ] Update `AGENTS.md` extracted crates section
- [ ] Update xtask crate list
- [ ] Regenerate `build-plan.json` with `unit2nix --workspace --force --no-check -o build-plan.json`
- [ ] Refresh generated docs with `cargo xtask docs`
- [ ] Refresh affected snapshots only if a rename changed user-visible TUI output
- [ ] `RUSTC_WRAPPER= cargo check && RUSTC_WRAPPER= cargo nextest run` — full workspace green
