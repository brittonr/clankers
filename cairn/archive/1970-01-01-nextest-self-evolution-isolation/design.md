# Design: Nextest Self-Evolution Isolation

## Context

The risky paths are subprocess-style tests that launch the compiled `clankers` binary. In `tests/self_evolution_cli.rs`, the command helper inherited the nextest process working directory and ambient HOME. The test targets were temporary files, but the launched process still ran from the checkout and could consult/write operator config or resolve future relative paths against the source tree.

In `tests/readiness_e2e.rs`, `ReadinessSandbox::clankers()` already provides a temporary workdir and HOME/XDG directories, but several fake-provider tool checks overrode `current_dir` back to `repo_root()`. That gave the read/find/bash/write/edit tool loop the real checkout as its ambient directory even though the test creates a fixture crate under the sandbox.

## Decisions

### Decision: subprocess integration tests own an isolated cwd and HOME

**Choice:** Self-evolution CLI tests call the binary through a helper that accepts a test cwd, sets that cwd to the temp directory, and points HOME/XDG config/cache/data/runtime variables at directories below the same temp tree.

**Rationale:** The self-evolution flow intentionally exercises dry-run, live apply, and rollback behavior. Even with absolute temp targets today, future relative paths or config lookups should resolve inside the disposable fixture rather than the Clankers checkout or the operator account.

### Decision: readiness tool tests use their sandbox fixture by default

**Choice:** Fake-provider readiness commands no longer override `ReadinessSandbox::clankers()` with `repo_root()` for bash/read/find/write/edit checks. The tests already create `Cargo.toml` and `src/nested/mod.rs` under the sandbox workdir, so tool behavior remains representative without exposing the source tree.

**Rationale:** The readiness suite should prove operator-visible tool behavior, not that the tools can operate on the real repository. The sandbox fixture gives stable, reviewable inputs and limits accidental mutation to disposable paths.

### Decision: do not add a global git-clean assertion to parallel nextest

**Choice:** This change avoids repo-root mutation authority instead of adding a broad `git status` assertion inside individual tests.

**Rationale:** Parallel tests may legitimately create ignored `.git/` checkpoint data or run against a locally dirty developer tree. A per-test global clean check would be flaky. The deterministic contract is that these tests do not use the repository as their writable working directory.

## Risks / Trade-offs

- The full `cargo nextest run` timeout in pi's Steel wrapper can still exceed 300 seconds even when nextest prints a passing summary; this change focuses on source-tree isolation, not runtime duration.
- Tests that wanted to read the real Clankers `Cargo.toml` now read the sandbox fixture. The assertion remains structural (`clankers` appears in the fixture package name) while avoiding checkout authority.

## Verification Plan

- Run `cargo nextest run --test self_evolution_cli --test readiness_e2e` to exercise the isolated CLI and fake-provider tool tests.
- Run `git diff --check` to catch whitespace errors.
- Run Cairn proposal/design/tasks gates and repository Cairn validation for the active change.
