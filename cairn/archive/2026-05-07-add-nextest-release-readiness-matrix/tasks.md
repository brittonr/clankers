## Phase 1: Specification foundation

- [x] [serial] Create the OpenSpec package for Rust/nextest release-readiness coverage gaps.

## Phase 2: Rust test harness foundation

- [x] [serial] Add shared Rust integration-test helpers for isolated `clankers` binary execution, temp homes/workdirs, bounded child-process timeouts, stdout/stderr capture, redacted diagnostics, and structured skip reasons. Evidence: `tests/readiness_common/mod.rs`; `cargo nextest run -p clankers --test readiness_e2e --test release_readiness_inventory --test readiness_opt_in --no-fail-fast` passed (7/7).
- [x] [depends:rust-test-helpers] Add a nextest-discoverable readiness inventory test that lists all default, opt-in, VM, live, and flake readiness tests and fails when a required readiness row is missing or only script-owned. Evidence: `tests/release_readiness_inventory.rs`; focused nextest passed (7/7).

## Phase 3: Credential-free E2E conversion

- [x] [depends:rust-test-helpers] Port fake/deterministic CLI E2E flows from `tests/e2e/run-tests.sh` into Rust integration tests covering version/config, fake print mode, read/write/edit tools, JSON output, and deterministic auth/status behavior without real credentials. Evidence: `tests/readiness_e2e.rs`; `./tests/e2e/run-tests.sh fake` passed (3/3).
- [x] [depends:e2e-rust-port] Demote the Bash E2E script to a compatibility wrapper or remove its release-readiness role, with docs pointing to the nextest filters. Evidence: `tests/e2e/run-tests.sh` delegates to `cargo nextest run`; `./scripts/test-harness.sh e2e fake` passed.

## Phase 4: Opt-in VM/live/flake readiness tests

- [x] [depends:readiness-inventory] Add Rust integration tests for live local-model/aspen2 Qwen readiness using existing availability probes, explicit opt-in gating, bounded timeouts, and no OAuth/browser flows. Evidence: `tests/readiness_opt_in.rs`; default gate skip path passed under focused nextest.
- [x] [depends:readiness-inventory] Add Rust integration tests that invoke the NixOS VM checks (`vm-smoke`, `vm-remote-daemon`, `vm-session-recovery`, `vm-plugin-runtime`, `vm-module-daemon`, `vm-module-router`, `vm-module-integration`) under explicit opt-in gating and assert command receipts rather than shell summaries. Evidence: `tests/readiness_opt_in.rs`; default gate skip path passed under focused nextest.
- [x] [depends:readiness-inventory] Add a Rust integration test for flake/CI readiness that invokes `nix flake check` under explicit opt-in gating and reports redacted bounded diagnostics. Evidence: `tests/readiness_opt_in.rs`; default gate skip path passed under focused nextest.

## Phase 5: Release-readiness wiring and verification

- [x] [depends:e2e-rust-port] Update release-readiness docs and any harness summary text so `cargo nextest` is the canonical readiness runner and Bash scripts are convenience wrappers only. Evidence: `README.md`, `docs/src/reference/release-readiness.md`, `scripts/test-harness.sh`; `cargo fmt --check` and `git diff --check` passed.
- [x] [depends:readiness-docs] Verify the default offline rail with `cargo fmt --check`, `cargo nextest run --workspace --no-fail-fast`, `cargo clippy --workspace --all-targets -- -D warnings`, and `./scripts/verify.sh`. Evidence: all commands passed; workspace nextest passed 3738/3738 with 2 skipped; clippy plus `./scripts/verify.sh` ended with `=== All checks passed ===`.
- [x] [depends:opt-in-tests] Verify at least one opt-in profile/filter path for VM/live/flake readiness or record explicit environmental blockers without marking those checks as passed. Evidence: `CLANKERS_RUN_LIVE_READINESS=1 cargo nextest run -p clankers --test readiness_opt_in readiness_live_local_model_aspen2_qwen36_nextest_opt_in --no-fail-fast` passed (1/1, 2 skipped).
