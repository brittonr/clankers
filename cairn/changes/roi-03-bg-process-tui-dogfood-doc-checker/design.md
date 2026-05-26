# Design: BG-process TUI dogfood docs checker

## Approach

- Treat `scripts/check-bg-process-tui-dogfood.rs` and `scripts/test-harness.sh dogfood bg-process-tui` as the executable source of truth.
- Check docs for command spelling plus required receipt facts: result pass, active process count/title, command visibility, layout toggle visibility, and cleanup.
- Prefer a small Rust test/checker near existing docs contract tests before adding another long-running harness step.
- Keep the checker deterministic and independent of running tmux; the live dogfood rail remains the runtime proof.

## Verification Plan

- Run `nix run .#cairn -- validate --root .`.
- Run proposal/design/tasks gates for this change and inspect JSON validity/verdict.
- Run the focused implementation checks named in `tasks.md` when draining the change.
- Run `git diff --check` before commit.
