## Why

Clankers OpenSpec reviews repeatedly find traceability omissions after proposal/design/tasks artifacts are already written. The current review metrics show the dominant repeated finding class is task-stage omissions: `omission|tasks|auto-fix` appears 491 times and deterministic-check omissions appear 151 times. These are process defects, not one-off wording defects.

This change hardens the OpenSpec gate/template workflow so future changes fail earlier when tasks omit spec-required contracts, deterministic fixture checks, or human/oracle checkpoint evidence.

## What Changes

- **Task traceability rail**: add a deterministic check or gate enhancement that compares task coverage claims against spec/design requirements and flags missing verification slices before implementation starts.
- **Deterministic-check template**: update OpenSpec task guidance so request/stream/retry/security contracts must name exact deterministic fixtures or checks instead of vague "test it" tasks.
- **Human/oracle checkpoint handling**: make repeated `human`-routed omission findings require explicit `H#` tasks plus checked-in `oracle-checkpoint` evidence, consistent with `openspec/AGENTS.md`.
- **Metrics-driven regression fixtures**: codify representative high-count review-metrics examples as safe fixtures so the rail catches the same omission classes without relying on live review memory.

## Non-Goals

- Rewriting all archived OpenSpec changes.
- Changing product behavior outside the OpenSpec/review workflow.
- Replacing human review; this change only makes recurring omissions mechanically visible earlier.

## Capabilities

### New Capabilities

- `openspec-review-gates`: deterministic OpenSpec artifact checks derived from repeated review findings.

### Modified Capabilities

- OpenSpec task authoring guidance and gate behavior for typed tasks, `[covers=...]`, deterministic fixture evidence, and `H#` oracle checkpoints.

## Impact

- **Files**: OpenSpec policy/gate helpers, OpenSpec guidance docs, tests/fixtures, review-metrics documentation.
- **APIs**: no runtime user-facing API changes expected.
- **Testing**: focused Rust/script tests for fixture detection, `openspec validate roi-01-harden-openspec-gate-omission-prevention --strict --json`, `git diff --check`, and any affected gate/check command.
