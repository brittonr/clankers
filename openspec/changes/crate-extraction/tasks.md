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

## Phase 5: llm-router (clankers-router)

Estimated effort: medium. Largest crate (16k lines), ALPN string rename,
config path migration, binary rename.

- [ ] Create `llm-router` repo on GitHub
- [ ] `git subtree split -P crates/clankers-router -b extract-router`
- [ ] Push split branch to new repo
- [ ] Rename crate and binary in Cargo.toml
- [ ] Global find-replace "clankers" in source (doc comments, error messages, ALPN)
- [ ] Replace `clankers/router/1` ALPN with `llm-router/1`
- [ ] Replace `~/.clankers/` config paths with XDG-compliant defaults
- [ ] Rename binary from `clankers-router` to `llm-router`
- [ ] Update `src/bin/clankers_router/` directory to `src/bin/llm_router/`
- [ ] Add README.md with provider list, architecture diagram, usage examples
- [ ] Add CI (the crate has `proxy`, `rpc`, `cli` features — test all combos)
- [ ] In clankers workspace: update `crates/clankers-router/Cargo.toml`
  to git dep on `llm-router` with `rpc` feature
- [ ] In clankers workspace: `pub use llm_router::*;` re-export
- [ ] Remove moved source files
- [ ] `cargo check && cargo nextest run`
- [ ] Verify all 26 importing files compile
- [ ] Verify `clankers-provider` re-exports still work
- [ ] Verify `clankers-message` `Usage` re-export still works
- [ ] Add ALPN compatibility: accept both old and new ALPN during transition
- [ ] Test iroh RPC connectivity with new ALPN

## Phase 6: ucan-cap (clankers-auth)

Estimated effort: medium-large. Requires generalizing Capability from
concrete enum to trait-based generic. Most invasive refactor.

- [ ] Create `ucan-cap` repo on GitHub
- [ ] Define `Capability` trait with `authorizes()` and `contains()` methods
- [ ] Define associated `Operation` type on the trait
- [ ] Make `CapabilityToken<C: Capability>` generic
- [ ] Make `TokenBuilder<C: Capability>` generic
- [ ] Make `TokenVerifier<C: Capability>` generic
- [ ] Extract `RevocationStore` trait (already exists, just move it)
- [ ] Write tests with a simple `TestCap` enum to validate the generic API
- [ ] `git subtree split` the generic modules
- [ ] Push to `ucan-cap` repo
- [ ] Strip all clankers references
- [ ] Add README.md, CI
- [ ] In clankers workspace: `crates/clankers-auth/` depends on `ucan-cap` via git dep
- [ ] Define `ClankerCapability` implementing `Capability` trait
- [ ] Define type alias `type ClankerToken = CapabilityToken<ClankerCapability>`
- [ ] Update all 7 call sites for the generic parameter
- [ ] Keep `RedbRevocationStore` in `clankers-auth` (redb is clankers-specific)
- [ ] `cargo check && cargo nextest run`
- [ ] Verify `src/commands/token.rs` compiles
- [ ] Verify `src/modes/daemon/` auth integration compiles
- [ ] Verify `src/modes/matrix_bridge/bot_commands.rs` compiles
- [ ] Run full authorization matrix tests

## Phase 7: Cleanup

After all extractions are stable:

- [ ] Grep workspace for remaining `clankers_merge`, `clankers_actor`, etc. imports
- [ ] Replace direct imports with extracted crate names where convenient
- [ ] Consider removing thin wrapper crates once all imports are migrated
- [ ] Update `AGENTS.md` architecture section to note extracted crates
- [ ] Update `openspec/config.yaml` context if needed
