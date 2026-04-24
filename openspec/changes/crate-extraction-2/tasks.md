# crate-extraction-2 — Tasks

> **Legend:** `[ ]` not started · `[~]` in progress ⏱ · `[x]` done ✅ `<duration>`
>
> **Status:** Reduced-scope change after the 2026-04-24 split. The remaining
> nix / matrix / zellij / protocol / db / hooks work moved to
> `crate-extraction-3`. This change now tracks only plugin-sdk, openspec,
> openspec-plugin, tui-types, message, and the cleanup for those extractions.

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
- [x] Update any existing plugins that depend on it (7 plugins + openspec-plugin)
- [x] Remove `crates/clankers-plugin-sdk/` from workspace

## Phase 2: specs (clankers-specs → openspec + openspec-plugin)

Infrastructure extraction with WASM plugin. Zero internal deps. 2 reverse deps.

### 2a: Library restructure and extraction

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
- [ ] Create `openspec` repo on GitHub (local repo exists at `~/git/openspec`; remote publishing still pending)
- [x] Push restructured code to the local sibling repo at `~/git/openspec`
- [x] Add README.md, LICENSE, CI
- [x] In workspace: add git dep, thin re-export wrapper
- [x] Remove moved source files
- [x] `cargo check && cargo test` on full workspace (195/196, 1 pre-existing tmux flake)

### 2b: WASM plugin

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
- [x] Write integration test: load plugin via extism, call each tool (`../openspec/openspec-plugin/tests/runtime.rs`; `cargo test --manifest-path openspec-plugin/Cargo.toml` passes with 3/3 tests on 2026-04-24)
- [x] Add plugin to clankers global plugins directory (`~/.clankers/agent/plugins/openspec/`)

## Phase 3: tui-types (clankers-tui-types → clanker-tui-types)

High-impact type extraction. Zero internal deps. 10 reverse deps.

### 3a: Resolve subwayrat path deps

- [x] Convert `rat-branches` path dep to git dep (subwayrat repo)
- [x] Convert `rat-leaderkey` path dep to git dep (subwayrat repo)
- [x] Convert `rat-widgets` path dep to git dep (subwayrat repo) if needed elsewhere
- [x] Verify workspace compiles with git deps instead of path deps

### 3b: Extract

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

### 3c: Migrate callers

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

## Phase 4: message (clankers-message → clanker-message)

High-impact type extraction. 1 internal dep (`clanker-router`). 7 reverse deps.

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

## Phase 5: Final cleanup

- [x] Grep workspace for any remaining `clankers_plugin_sdk`, `clankers_specs`, `clankers_tui_types`, `clankers_message`
- [x] Remove all thin wrapper crates for the reduced-scope extractions
- [x] Update workspace `members` list in root Cargo.toml
- [x] Update `AGENTS.md` extracted crates section
- [x] Update xtask crate list
- [x] `RUSTC_WRAPPER= cargo check && RUSTC_WRAPPER= cargo nextest run` — full workspace green (1129 passed on 2026-04-24)
- [x] Verify openspec plugin loads and all 5 tools work (`../openspec/openspec-plugin/tests/runtime.rs`; `cargo test --manifest-path openspec-plugin/Cargo.toml` passes with 3/3 tests on 2026-04-24; supplemental host-runtime smoke via `/tmp/verify_openspec_plugin.rs`)
