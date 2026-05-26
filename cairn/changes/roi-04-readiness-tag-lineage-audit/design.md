# Design: Readiness tag lineage audit

## Approach

- Use `git show-ref --tags`/`git rev-list` and latest harness receipts as the factual source.
- Represent each tag as immutable checkpoint evidence with target commit and key delta.
- Avoid self-referential current-HEAD wording in checked-in docs.
- Keep verification docs-focused and cheap.

## Verification Plan

- Run `nix run .#cairn -- validate --root .`.
- Run proposal/design/tasks gates for this change and inspect JSON validity/verdict.
- Run the focused implementation checks named in `tasks.md` when draining the change.
- Run `git diff --check` before commit.
