# Design: Dogfood full-readiness evidence page

## Approach

- Record the evaluated payload commit and tag target explicitly.
- Index `target/test-harness/runs/<run_id>/results.json`, summary/JUnit aliases, and the dogfood receipt path as local evidence paths.
- State scope boundaries: internal/trusted operator readiness, not broad provider stability or public unattended production.
- Use docs-only verification (`mdbook build docs`, grep/contract checks, `git diff --check`).

## Verification Plan

- Run `nix run .#cairn -- validate --root .`.
- Run proposal/design/tasks gates for this change and inspect JSON validity/verdict.
- Run the focused implementation checks named in `tasks.md` when draining the change.
- Run `git diff --check` before commit.
