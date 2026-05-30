Evidence-ID: repro-audit
Artifact-Type: investigation-note
Task-ID: R1
Covers: r[nextest-self-evolution-isolation.self-evolution.isolated-subprocesses], r[nextest-self-evolution-isolation.readiness.sandboxed-tool-cwd]
Created: 2026-05-30
Status: complete

# Reproduction and Seam Audit

## Scope

Investigated the source-tree mutation risk after a broad `cargo nextest run` was observed with unrelated source diffs in the main checkout.

## Findings

- Current main already had one unpushed cleanup commit before this change started: `300f4288 Clean up UCAN auth validation fallout`.
- A detached sibling worktree at `/home/brittonr/git/clankers-nextest-repro` was created from `origin/main` so reproduction would not modify the active checkout.
- Focused `self_evolution_cli` nextest in that sibling worktree passed and left the sibling worktree clean.
- A partial broad nextest run in that sibling worktree failed early in `daemon_tool_rebuilder_filters_plugin_tools` before reaching the long tail, and still left the sibling worktree clean.
- Code audit found two isolation gaps even though the focused reproduction did not dirty the tree:
  - `tests/self_evolution_cli.rs` launched `clankers` from the inherited nextest cwd and ambient HOME.
  - `tests/readiness_e2e.rs` created a sandbox fixture but overrode several fake-provider tool commands back to `repo_root()`.

## Machine Evidence

Commands run:

```text
git worktree add --detach /home/brittonr/git/clankers-nextest-repro origin/main
cd /home/brittonr/git/clankers-nextest-repro && TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run --test self_evolution_cli
git -C /home/brittonr/git/clankers-nextest-repro status --short --branch
cd /home/brittonr/git/clankers-nextest-repro && TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run
git -C /home/brittonr/git/clankers-nextest-repro status --short --branch
```

Result excerpts:

```text
Nextest run ID 529cf41e-1804-4a01-9550-20f5cbe6cb4f
Starting 2 tests across 1 binary
PASS clankers::self_evolution_cli self_evolution_cli_rejects_stale_target_live_apply_before_mutation
PASS clankers::self_evolution_cli self_evolution_cli_runs_approve_preflight_and_live_apply_with_temp_files
Summary [0.500s] 2 tests run: 2 passed, 0 skipped

git status after focused run:
## HEAD (no branch)
```

The partial broad run built the full test binary and then failed before completion:

```text
FAIL clankers modes::daemon::agent_process::factory_plugin_tests::daemon_tool_rebuilder_filters_plugin_tools
assertion failed: all_names.contains(&"test_echo".to_string())
Summary [6.280s] 201/1520 tests run: 200 passed, 1 failed, 0 skipped

git status after partial broad run:
## HEAD (no branch)
```
