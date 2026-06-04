## Context

`./scripts/test-harness.sh full` currently runs fmt, cargo check, workspace nextest, clippy, repo verify, and tigerstyle. It does not run the separate `e2e`, `live`, `vm`, or `ci` modes by default. The `e2e` mode delegates to `tests/e2e/run-tests.sh`, while live and many workspace tests already have Rust integration-test shape. The user wants the missing surfaces included as hardened Rust tests under `cargo nextest`, not as Bash scripting.

## Goals / Non-Goals

**Goals:**
- Move release-readiness coverage into Rust integration tests runnable by `cargo nextest`.
- Cover credential-free E2E, live provider smoke, VM check dispatch, and flake/CI readiness as named nextest tests or profiles.
- Make skipped host-dependent tests explicit, bounded, and auditable rather than silently absent.
- Preserve default offline safety: network/live/VM/flake-heavy checks must require explicit opt-in or self-skip with a clear reason.

**Non-Goals:**
- Running live OAuth/provider credential flows by default.
- Replacing NixOS VM implementations themselves with pure Rust; Rust tests may invoke Nix checks as controlled child processes.
- Removing convenience shell wrappers immediately, unless equivalent Rust tests make them redundant and docs point at nextest.

## Decisions

### 1. Nextest is the canonical readiness runner

**Choice:** All new readiness coverage SHALL be represented as Rust tests that `cargo nextest` can discover and run/filter.

**Rationale:** Nextest gives stable test inventory, filtering, sharding, retries, JUnit output, timeout policy, and integration with the existing workspace test rail.

**Alternative:** Keep broad readiness in Bash selectors. Rejected because it recreates the current gap and cannot prove coverage through the existing nextest workspace receipt.

**Implementation:** Add Rust integration tests for CLI E2E behavior, harness/coverage inventory, live provider availability contracts, VM check dispatch, and flake check dispatch. Bash wrappers may call nextest filters, not own assertions.

### 2. Host-dependent checks are opt-in but still visible

**Choice:** VM, live, and flake-heavy checks SHALL be Rust tests with explicit env/profile gates and clear skip diagnostics when prerequisites are absent.

**Rationale:** Default developer/CI runs must remain safe and credential-free, but the coverage should not disappear from test inventory.

**Alternative:** Mark tests `#[ignore]` only. Rejected as too easy to miss; ignored tests need docs/profile wiring and an inventory guard.

**Implementation:** Use bounded `std::process::Command` helpers with explicit env vars such as `CLANKERS_RUN_VM_READINESS=1`, `CLANKERS_RUN_LIVE_READINESS=1`, and `CLANKERS_RUN_FLAKE_READINESS=1`, plus deterministic skip messages when unset or when non-required hosts are unavailable.

### 3. Bash E2E assertions become Rust binary tests

**Choice:** The fake/deterministic CLI E2E flows SHALL be ported to Rust integration tests that run the built binary with isolated config/home/temp directories.

**Rationale:** Rust tests can assert exit status, stdout/stderr, file effects, JSON structure, and redaction without shell parsing pitfalls.

**Alternative:** Wrap `tests/e2e/run-tests.sh` from a Rust test. Rejected for final acceptance because it preserves Bash as the assertion engine.

**Implementation:** Factor small test helpers for isolated `CLANKERS_FAKE_PROVIDER=1`, `CLANKERS_NO_DAEMON=1`, temp workspace files, stdout/stderr capture, and timeout enforcement.

## Risks / Trade-offs

**Expensive checks in default nextest** → Keep VM/live/flake-heavy checks opt-in or profile-gated while making their inventory explicit.

**Skip-as-pass overclaiming** → Add an inventory/contract test that fails if opt-in readiness tests are missing, undocumented, or not named/filterable; docs must distinguish default offline pass from opt-in readiness pass.

**Command child-process flakiness** → Use bounded timeouts, isolated temp dirs, redacted logs, and exact expected outputs rather than broad substring-only success.
