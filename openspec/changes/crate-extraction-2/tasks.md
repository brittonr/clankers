# crate-extraction-2 — Tasks

> **Legend:** `[ ]` not started · `[~]` in progress ⏱ · `[x]` done ✅ `<duration>`

## Phase 1: plugin-sdk (clankers-plugin-sdk → clanker-plugin-sdk) ✅

Completed. Repo: github.com/brittonr/clanker-plugin-sdk

- [x] Create `clanker-plugin-sdk` repo on GitHub
- [x] Copy `crates/clankers-plugin-sdk/` contents to new repo
- [x] Rename crate in Cargo.toml (`name = "clanker-plugin-sdk"`)
- [x] Replace all `clankers_plugin_sdk` / `clankers-plugin-sdk` references in source and docs
- [x] Verify `cargo build --target wasm32-unknown-unknown` compiles
- [x] Add README.md with quick-start example (from existing lib.rs docs)
- [x] Add LICENSE
- [x] Add CI (cargo check, clippy, fmt, cargo build --target wasm32)
- [x] Update any existing plugins that depend on the path dep (7 plugins + openspec-plugin)
- [x] Remove `crates/clankers-plugin-sdk/` from workspace

## Phase 2: nix (clankers-nix → clanker-nix)

Leaf extraction. Zero internal deps. snix git deps carry over.

- [ ] Create `clanker-nix` repo on GitHub
- [ ] `git subtree split -P crates/clankers-nix -b extract-nix`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-nix"`)
- [ ] Replace all `clankers_nix` / `clankers-nix` references in source
- [ ] Preserve feature flags: `eval`, `refscan`
- [ ] Preserve snix rev pin (`8fe3bade...`)
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 3: matrix (clankers-matrix → clanker-matrix)

Leaf extraction. Zero internal deps. Heavy external deps (matrix-sdk).

- [ ] Create `clanker-matrix` repo on GitHub
- [ ] `git subtree split -P crates/clankers-matrix -b extract-matrix`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-matrix"`)
- [ ] Replace all `clankers_matrix` / `clankers-matrix` references in source
- [ ] Preserve matrix-sdk features: `e2e-encryption`, `sqlite`, `rustls-tls`
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 4: zellij (clankers-zellij → clanker-zellij)

Leaf extraction. Zero internal deps. iroh QUIC dep.

- [ ] Create `clanker-zellij` repo on GitHub
- [ ] `git subtree split -P crates/clankers-zellij -b extract-zellij`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-zellij"`)
- [ ] Replace all `clankers_zellij` / `clankers-zellij` references in source
- [ ] Preserve iroh version and `address-lookup-mdns` feature
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run` on full workspace

## Phase 5: protocol (clankers-protocol → clanker-protocol)

Infrastructure extraction. Zero internal deps. 2 reverse deps.

- [ ] Create `clanker-protocol` repo on GitHub
- [ ] `git subtree split -P crates/clankers-protocol -b extract-protocol`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-protocol"`)
- [ ] Replace all `clankers_protocol` / `clankers-protocol` references in source
- [ ] Verify frame, command, control, event, types modules all compile
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run` — verify root crate + controller compile

## Phase 6: specs (clankers-specs → openspec + openspec-plugin)

Infrastructure extraction with WASM plugin. Zero internal deps. 2 reverse deps.

### 6a: Library restructure and extraction

- [x] Restructure clankers-specs into core/ (pure, no std::fs) and engine (std::fs)
- [x] Move `parse_spec_content`, `parse_scenarios`, `detect_strength` to core
- [x] Move `ArtifactGraph::from_state` (takes existing_files list, not Path) to core
- [x] Move `TaskProgress` parsing from string content to core
- [x] Move `parse_delta_content` to core
- [x] Move `verify_from_content` (takes strings, not paths) to core
- [x] Move `Schema`, `SchemaArtifact`, templates to core
- [x] Add `fs` feature flag gating `SpecEngine` and `config.rs`
- [x] Verify `cargo check --no-default-features` compiles (core only, no std::fs)
- [x] Verify `cargo check` compiles (full, with SpecEngine)
- [x] All existing tests pass (63 tests: 34 pure + 29 fs)
- [ ] Create `openspec` repo on GitHub
- [x] Push restructured code (local at ~/git/openspec)
- [x] Add README.md, LICENSE, CI
- [x] In workspace: add git dep, thin re-export wrapper
- [x] Remove moved source files
- [x] `cargo check && cargo test` on full workspace (195/196, 1 pre-existing tmux flake)

### 6b: WASM plugin

- [x] Create `openspec-plugin/` directory in openspec repo
- [x] Add Cargo.toml: cdylib target, deps on openspec (no fs) + clanker-plugin-sdk
- [x] Implement `describe()` returning PluginMeta with 5 tools
- [x] Implement `handle_tool_call` dispatcher for 5 tools
- [x] Implement `spec_list` handler: parse entries, return spec summaries
- [x] Implement `spec_parse` handler: parse markdown content → structured spec
- [x] Implement `change_list` handler: parse change entries with task progress
- [x] Implement `change_verify` handler: verify tasks content + specs presence
- [x] Implement `artifact_status` handler: build graph from schema + existing files
- [x] Implement `on_event` handler for `agent_start`
- [x] Write `plugin.json` manifest with tool_definitions and input schemas
- [x] `cargo build --target wasm32-unknown-unknown` succeeds (847K release binary)
- [ ] Write integration test: load plugin via extism, call each tool
- [x] Add plugin to clankers global plugins directory (~/.clankers/agent/plugins/openspec/)

## Phase 7: db (clankers-db → clanker-db)

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
- [ ] `cargo check && cargo nextest run` — verify root crate + agent compile

## Phase 8: hooks (clankers-hooks → clanker-hooks)

Infrastructure extraction. Zero internal deps. 5 reverse deps.

- [ ] Create `clanker-hooks` repo on GitHub
- [ ] `git subtree split -P crates/clankers-hooks -b extract-hooks`
- [ ] Push split branch to new repo
- [ ] Rename crate in Cargo.toml (`name = "clanker-hooks"`)
- [ ] Replace all `clankers_hooks` / `clankers-hooks` references in source
- [ ] Add `Custom(String)` variant to `HookPoint` for extensibility
- [ ] Verify config, dispatcher, git, payload, point, script, verdict modules compile
- [ ] Add README.md, LICENSE, CI
- [ ] In workspace: add git dep, thin re-export wrapper
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run` — verify all 5 reverse deps compile

## Phase 9: tui-types (clankers-tui-types → clanker-tui-types)

High-impact type extraction. Zero internal deps. 10 reverse deps.

### 9a: Resolve subwayrat path deps

- [x] Convert `rat-branches` path dep to git dep (subwayrat repo)
- [x] Convert `rat-leaderkey` path dep to git dep (subwayrat repo)
- [x] Convert `rat-widgets` path dep to git dep (subwayrat repo) if needed elsewhere
- [x] Verify workspace compiles with git deps instead of path deps

### 9b: Extract

- [x] Create `clanker-tui-types` repo on GitHub
- [x] `git subtree split -P crates/clankers-tui-types -b extract-tui-types`
- [x] Push split branch to new repo
- [x] Rename crate in Cargo.toml (`name = "clanker-tui-types"`)
- [x] Replace all `clankers_tui_types` / `clankers-tui-types` references in source
- [x] Verify all 18 type modules compile
- [x] Add README.md, LICENSE, CI
- [x] In workspace: add git dep, thin re-export wrapper
- [x] Remove moved source files
- [x] `cargo check && cargo nextest run` — verify all 10 reverse deps compile

### 9c: Migrate callers

- [x] Migrate root crate imports to `clanker_tui_types`
- [x] Migrate clankers-agent imports
- [x] Migrate clankers-config imports
- [x] Migrate clankers-controller imports
- [x] Migrate clankers-model-selection imports
- [x] Migrate clankers-plugin imports
- [x] Migrate clankers-procmon imports
- [x] Migrate clankers-provider imports
- [x] Migrate clankers-tui imports
- [x] Migrate clankers-util imports
- [x] Remove thin wrapper crate from workspace

## Phase 10: message (clankers-message → clanker-message)

High-impact type extraction. 1 internal dep (clanker-router). 7 reverse deps.

- [x] Create `clanker-message` repo on GitHub
- [x] `git subtree split -P crates/clankers-message -b extract-message`
- [x] Push split branch to new repo
- [x] Rename crate in Cargo.toml (`name = "clanker-message"`)
- [x] Convert clanker-router workspace dep to git dep in new repo
- [x] Replace all `clankers_message` / `clankers-message` references in source
- [x] Add README.md, LICENSE, CI
- [x] In workspace: add git dep, thin re-export wrapper
- [x] Remove moved source files
- [x] `cargo check && cargo nextest run` — verify all 7 reverse deps compile
- [x] Migrate all 7 callers from `clankers_message` to `clanker_message`
- [x] Remove thin wrapper crate from workspace

## Phase 11: Final cleanup

- [ ] Grep workspace for any remaining `clankers_plugin_sdk`, `clankers_nix`,
      `clankers_matrix`, `clankers_zellij`, `clankers_protocol`, `clankers_specs`,
      `clankers_db`, `clankers_hooks`, `clankers_tui_types`, `clankers_message`
- [ ] Remove all remaining thin wrapper crates
- [ ] Update workspace `members` list in root Cargo.toml
- [ ] Update `AGENTS.md` extracted crates section
- [ ] Update xtask crate list
- [ ] `cargo check && cargo nextest run` — full workspace green
- [ ] Verify openspec plugin loads and all 5 tools work
