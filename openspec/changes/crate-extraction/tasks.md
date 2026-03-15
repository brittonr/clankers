# crate-extraction — Tasks

## Phase 1: graggle (clankers-merge) ✅

Completed. Repo: github.com/brittonr/graggle

- [x] Create `graggle` repo on GitHub
- [x] `git subtree split -P crates/clankers-merge -b extract-merge`
- [x] Push split branch to new repo
- [x] Rename crate in Cargo.toml (`name = "graggle"`)
- [x] Replace 2 "clankers" references in `lib.rs` doc comments
- [x] Update `use clankers_merge::` in doc example to `use graggle::`
- [x] Add README.md with theory background and usage example
- [x] Add CI (cargo test, clippy, fmt, nextest)
- [x] In clankers workspace: update `crates/clankers-merge/Cargo.toml` to git dep on `graggle`
- [x] In clankers workspace: replace `src/lib.rs` with `pub use graggle::*;`
- [x] Remove moved source files from workspace
- [x] `cargo check && cargo nextest run` on full workspace
- [x] Verify `verus/merge_spec.rs` still compiles (standalone verus file, no import change needed)
- [x] Verify `src/worktree/merge_strategy.rs` still compiles

## Phase 2: clanker-actor (clankers-actor) ✅

Completed. Repo: github.com/brittonr/clanker-actor

- [x] Create `clanker-actor` repo on GitHub
- [x] `git subtree split -P crates/clankers-actor -b extract-actor`
- [x] Push split branch to new repo
- [x] Rename crate in Cargo.toml (`name = "clanker-actor"`)
- [x] Fix 1 "clankers" reference in `registry.rs` doc comment
- [x] Update integration test imports from `clankers_actor` to `clanker_actor`
- [x] Add README.md
- [x] Add LICENSE (MIT)
- [x] Add CI (cargo test, clippy, fmt, nextest)
- [x] In clankers workspace: update `crates/clankers-actor/Cargo.toml` to git dep
- [x] In clankers workspace: replace `src/lib.rs` with `pub use clanker_actor::*;`
- [x] Remove moved source files + integration tests
- [x] `cargo check` on full workspace
- [x] Verify all 14 call sites compile (daemon, controller, matrix bridge, subagent)

## Phase 3: clanker-scheduler (clankers-scheduler) ✅

Completed. Repo: github.com/brittonr/clanker-scheduler

- [x] Create repo on GitHub
- [x] `git subtree split -P crates/clankers-scheduler -b extract-scheduler`
- [x] Push split branch to new repo
- [x] Rename crate (`name = "clanker-scheduler"`)
- [x] Strip 1 "clankers" reference in `lib.rs`
- [x] Add README.md, LICENSE, CI
- [x] In clankers workspace: thin wrapper with git dep
- [x] Remove moved source files
- [x] `cargo check` on full workspace
- [x] Verify `src/tools/schedule.rs` compiles and tests pass

## Phase 4: clanker-loop (clankers-loop) ✅

Completed. Repo: github.com/brittonr/clanker-loop

- [x] Create repo on GitHub
- [x] `git subtree split -P crates/clankers-loop -b extract-loop`
- [x] Push split branch to new repo
- [x] Rename crate (`name = "clanker-loop"`)
- [x] Rewrite "clankers" references in `lib.rs` and `truncation.rs`
- [x] Add README.md, LICENSE, CI
- [x] In clankers workspace: thin wrapper with git dep
- [x] Remove moved source files
- [x] `cargo check` on full workspace
- [x] Verify all callers compile (controller, agent, schedule tool)

## Phase 5: clanker-router (clankers-router) ✅

Completed. Repo: github.com/brittonr/clanker-router

- [x] Create repo on GitHub
- [x] `git subtree split -P crates/clankers-router -b extract-router`
- [x] Push split branch to new repo
- [x] Rename crate and binary (`clanker-router`)
- [x] Global find-replace: 93 "clankers" references across 13 files
- [x] ALPN: `clankers/router/1` → `clanker/router/1`
- [x] ALPN: `clankers-router-http/1` → `clanker-router-http/1`
- [x] Config paths: `~/.config/clankers-router/` → `~/.config/clanker-router/`
- [x] Cache paths: `~/.cache/clankers-router/` → `~/.cache/clanker-router/`
- [x] mDNS: `_clankers-router._udp.local` → `_clanker-router._udp.local`
- [x] Binary directory: `src/bin/clankers_router/` → `src/bin/clanker_router/`
- [x] Add README.md, LICENSE, CI
- [x] In workspace: thin wrapper with `rpc` feature, re-export
- [x] Remove all moved source files (39 .rs files)
- [x] `cargo check` on full workspace — all 26 importing files compile
- [x] Re-export chain works: clankers-provider, clankers-message, etc.

## Phase 6: clanker-auth (clankers-auth) ✅

Completed. Repo: github.com/brittonr/clanker-auth

- [x] Create repo on GitHub
- [x] Define `Cap` trait with `authorizes()`, `contains()`, `is_delegate()`
- [x] Define associated `Operation` type on the trait
- [x] Make `CapabilityToken<C: Cap>` generic (with `#[serde(bound)]`)
- [x] Make `TokenBuilder<C: Cap>` generic
- [x] Make `TokenVerifier<C: Cap>` generic
- [x] Move `RevocationStore` trait to extracted crate
- [x] Write 14 tests with `TestCap` enum validating generic API
- [x] Push to `clanker-auth` repo with README, LICENSE, CI
- [x] In workspace: `Capability` enum implements `Cap` trait
- [x] Type aliases: `CapabilityToken`, `TokenBuilder`, `TokenVerifier`
- [x] `constants.rs` and `utils.rs` re-export from `clanker-auth`
- [x] `RedbRevocationStore` stays in `clankers-auth` (uses `clanker_auth::RevocationStore`)
- [x] `generate_root_token()` stays in `clankers-auth`
- [x] All 52 existing tests pass unchanged
- [x] All 5 call sites compile (token.rs, session_store.rs, handlers.rs, bot_commands.rs, path_policy.rs)

## Phase 7: Cleanup ✅

Completed. All thin wrapper crates removed, imports point directly to extracted crates.

- [x] Grep workspace for remaining `clankers_merge`, `clankers_actor`, etc. imports
- [x] Replace direct imports with extracted crate names where convenient
- [x] Remove thin wrapper crates (merge, actor, scheduler, loop, router) — all imports migrated
- [x] Update `AGENTS.md` architecture section to note extracted crates
- [x] Update filesystem paths (`~/.config/clankers-router/` → `~/.config/clanker-router/`)
- [x] Update Verus spec comments to reference new crate names
- [x] Update xtask crate list
- [x] `cargo check` + `cargo nextest run` — 576 passed, 0 failed
