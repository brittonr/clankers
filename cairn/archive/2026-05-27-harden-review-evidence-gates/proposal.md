## Why

Review metrics still show repeated omission findings after the existing project-local review-gate hardening slices. The highest current category is `omission|spec|prompt`: proposal or design artifacts state strong lifecycle constraints, but the delta spec either omits them or weakens them into generic, optional, or unrelated wording. Representative safe examples include generated artifact hygiene not being traceable to any delta requirement, required local verification becoming optional generic evidence, and a no-GitHub constraint being weakened from forbidden to merely not required.

These are expensive late-stage review WARNs because authors believe the change is specified, reviewers cannot mechanically trace the promise, and the same artifact-shape correction repeats across changes.

## What Changes

- Add a narrow lifecycle change for strong proposal/design constraints that must be preserved in delta specs.
- Preserve a sanitized review-metrics snapshot for the selected category under this change.
- Plan project-local fixture/checker/guidance updates that reject missing or weakened strong constraints before implementation tasks close.
- Keep the first implementation in the Clankers repo-local review-gate rail; do not change generic Cairn core until the local rule shape is proven.

## Impact

- **Files**: `scripts/check-openspec-review-gates.rs`, `scripts/fixtures/openspec-review-gates/*`, `docs/src/reference/openspec-review-gates.md`, `cairn/specs/openspec-review-gates/spec.md`, and this change package.
- **Testing**: Run the focused review-gate checker, docs build, Cairn proposal/design/tasks gates, Cairn validation, and whitespace diff checks.
- **Non-goals**: no live provider probes, no raw private transcripts, no hidden prompts, no credentials, no generic Cairn-core gate change in this slice.
