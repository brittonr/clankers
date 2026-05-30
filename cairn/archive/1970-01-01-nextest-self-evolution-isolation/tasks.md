# Tasks: Nextest Self-Evolution Isolation

## Phase 0: Audit

- [x] [serial] R1. Audit the suspected nextest source-tree mutation seams and reproduce focused self-evolution CLI behavior in a sibling worktree. [covers=r[nextest-self-evolution-isolation.self-evolution.isolated-subprocesses], r[nextest-self-evolution-isolation.readiness.sandboxed-tool-cwd]] [evidence=evidence/repro-audit.md]

## Phase 1: Implementation

- [x] [serial] I1. Run self-evolution CLI subprocess commands from a fixture temp cwd with fixture-local HOME/XDG paths. [covers=r[nextest-self-evolution-isolation.self-evolution.isolated-subprocesses]]
- [x] [serial] I2. Keep fake-provider readiness tool loops inside `ReadinessSandbox` workdirs instead of overriding them to `repo_root()`. [covers=r[nextest-self-evolution-isolation.readiness.sandboxed-tool-cwd]]

## Phase 2: Verification

- [x] [serial] V1. Run focused nextest rails for `self_evolution_cli` and `readiness_e2e` plus `git diff --check`. [covers=r[nextest-self-evolution-isolation.verification.focused-rails]] [evidence=evidence/focused-validation.md]
- [x] [serial] V2. Run Cairn proposal/design/tasks gates and repository Cairn validation. [covers=r[nextest-self-evolution-isolation.verification.focused-rails]] [evidence=evidence/cairn-validation.md]
