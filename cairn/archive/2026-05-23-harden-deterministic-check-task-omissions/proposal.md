## Why

Review metrics still show `omission|tasks|deterministic-check` as the largest remaining task-omission category after the auto-fix rail landed. The repeated examples share one root cause: task ledgers say "test" or "verify" a deterministic contract, but do not name the concrete fixture, helper, command, golden file, script, or evidence path that would make the check reproducible.

## What Changes

- Add a project-local review-gate diagnostic for vague deterministic-check tasks.
- Add sanitized positive and negative fixtures that cover the repeated omission pattern.
- Update operator guidance and the accepted `openspec-review-gates` spec so future changes know the required task shape.
- Preserve a sanitized metrics snapshot as reviewable evidence for the selected category.

## Impact

- **Files**: `scripts/check-openspec-review-gates.rs`, `scripts/fixtures/openspec-review-gates/*`, `docs/src/reference/openspec-review-gates.md`, `cairn/specs/openspec-review-gates/spec.md`, and this change package.
- **Testing**: Run the focused review-gate checker, docs build, Cairn proposal/design/tasks gates, Cairn validation, and whitespace diff checks.
