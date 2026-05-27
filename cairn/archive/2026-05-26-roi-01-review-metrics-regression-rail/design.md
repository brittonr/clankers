# Design: Review metrics regression rail

## Approach

- Use `scripts/check-cairn-review-gates.rs` and its fixture tree as the first implementation surface because Clankers owns the observed category vocabulary and repo-local examples.
- Create positive and negative fixtures for the top repeated categories first: task auto-fix omissions, deterministic-check omissions, and task prompt traceability omissions.
- Keep evidence sanitized: counts, category keys, source/stage labels, and short behavior summaries only.
- Emit deterministic issue codes so future metrics can show whether repeated categories are blocked by a rail rather than rediscovered in review.

## Verification Plan

- Run `nix run .#cairn -- validate --root .`.
- Run proposal/design/tasks gates for this change and inspect JSON validity/verdict.
- Run the focused implementation checks named in `tasks.md` when draining the change.
- Run `git diff --check` before commit.
