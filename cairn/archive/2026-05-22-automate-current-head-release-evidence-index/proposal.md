# Automate current-HEAD release evidence index

## Summary
Add a first-class local evidence-index rail that composes existing test-harness receipts with Git/lifecycle state for the current Clankers checkout. The rail should fail closed by default on dirty worktrees, missing receipt artifacts, failed receipt steps, and stale lifecycle state, while writing deterministic Markdown and JSON index artifacts under `target/`.

## Motivation
Clankers already emits harness receipts for quick/full/live/VM/CI profiles, but current-HEAD readiness promotion is still manual. That makes docs and tags easy to stale after a green live run. A small generator/harness mode gives operators one command that captures the payload HEAD, tag distance, branch/upstream state, lifecycle state, latest valid receipts, and explicit non-claims.

## Scope
- Add an operator command reachable through `./scripts/test-harness.sh evidence-index`.
- Add a Rust-owned script that gathers Git state and harness receipts, validates referenced artifacts, and writes deterministic index artifacts.
- Update harness inventory and contract tests.
- Keep raw generated evidence under ignored `target/`; do not check in current-HEAD receipt output in this slice.

## Non-goals
- Do not move readiness tags automatically.
- Do not publish artifacts remotely.
- Do not claim full/VM/CI/live readiness unless matching passed receipts exist.
- Do not make Bash the assertion source of truth beyond invoking the Rust script.
