# Testing Specification

## Purpose

Ensure nextest-owned CLI/tool integration tests exercise self-evolution and fake-provider tool behavior without granting those tests ambient write authority over the Clankers checkout.

## Requirements

### Requirement: Isolated self-evolution CLI subprocesses
r[nextest-self-evolution-isolation.self-evolution.isolated-subprocesses] Self-evolution CLI tests MUST launch the `clankers` binary from a per-test temporary working directory and MUST use an isolated HOME/XDG environment under that temporary directory.

#### Scenario: Run self-evolution apply and rollback fixture
r[nextest-self-evolution-isolation.self-evolution.isolated-subprocesses.apply-rollback]
- GIVEN a self-evolution test fixture with temporary target, candidate, receipt, approval, and application paths
- WHEN the test runs `self-evolution run`, `approve`, `apply`, and `rollback`
- THEN command cwd and HOME/XDG paths are scoped to the fixture temp directory
- AND live apply/rollback mutate only the fixture target and receipts

### Requirement: Readiness fake-provider tool sandboxing
r[nextest-self-evolution-isolation.readiness.sandboxed-tool-cwd] Fake-provider readiness tests MUST use `ReadinessSandbox` workdirs for tool loops instead of overriding command cwd to the repository root.

#### Scenario: Run fake-provider tool loop
r[nextest-self-evolution-isolation.readiness.sandboxed-tool-cwd.tool-loop]
- GIVEN a readiness sandbox containing fixture `Cargo.toml` and `src/` files
- WHEN fake-provider prompts request bash, read, find, write, edit, and read-back tool operations
- THEN those commands resolve relative paths from the sandbox workdir
- AND absolute write/edit fixtures remain outside the Clankers checkout

### Requirement: Focused verification
r[nextest-self-evolution-isolation.verification.focused-rails] The change MUST be verified with focused nextest rails for the subprocess/tool tests and Cairn gates for the lifecycle package.

#### Scenario: Run verification rails
r[nextest-self-evolution-isolation.verification.focused-rails.run]
- GIVEN the implementation changes are present
- WHEN focused nextest, `git diff --check`, and Cairn validation/gates run
- THEN all checks pass or any failures are recorded as explicit follow-up evidence
