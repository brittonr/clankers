# Design: Steel eval controlled corpus dogfood

## Context

`steel_eval` is available, but promotion beyond mechanical availability needs measured dogfood over a reviewed local corpus/profile with thresholds, regression budget, and safe receipts.

## Approach

- Use local manifest-shaped corpus input analogous to self-evolution productionization: version, targets, cases, redaction policy, minimum improvement, and regression budget.
- Keep Rust as the owner of evaluation, thresholds, receipt shape, and authority decisions; Steel remains evaluated through the existing wrapper/profile boundary.
- Start with fixtures and fake/local corpus evidence before broad live dogfood.

## Verification

- Validate this Cairn package with repo-local/native Cairn validation.
- Run proposal, design, and tasks gates and inspect `valid`/`verdict` receipts.
- Run the implementation-specific verification named in `tasks.md` when draining this package.
