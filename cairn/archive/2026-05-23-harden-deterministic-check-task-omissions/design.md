## Context

The previous review-metrics rail added `missing-auto-fix-task` for `omission|tasks|auto-fix`. Current promotion data still reports `omission|tasks|deterministic-check` with 151 repeated findings. Existing deterministic contract categories already catch specific subcontracts such as request shape, stream boundaries, retry policy, default/override, active account persistence, entitlement probes, and tool-call deltas, but they do not fail a task ledger that acknowledges a deterministic check obligation generically without naming the concrete reproducible artifact.

## Decisions

### 1. Add a generic deterministic-check task-shape diagnostic

**Choice:** Add `missing-deterministic-check-artifact-task` as a task-stage diagnostic triggered by artifact text that requires deterministic fixture/check coverage while `tasks.md` lacks a task line naming both deterministic-check intent and a concrete fixture/helper/command/golden/script/evidence path.

**Rationale:** The repeated findings are not about one product feature; they are about the task shape required to make deterministic checks reviewable. A generic diagnostic prevents the same omission across request fixtures, entitlement probe fixtures, stream parser fixtures, docs/help fixtures, and future deterministic rails.

**Alternative:** Add only another product-specific category. Rejected because metrics examples span several feature surfaces and would keep requiring one-off diagnostics.

### 2. Keep the implementation project-local

**Choice:** Implement the Clankers-specific wording in `scripts/check-openspec-review-gates.rs` and fixtures, while relying on central Cairn only for lifecycle gates.

**Rationale:** The central Cairn review-metrics rail is already accepted. This slice tunes Clankers' checker vocabulary and fixture corpus.

### 3. Use sanitized fixtures as the regression boundary

**Choice:** Add one negative fixture with vague deterministic-check tasks and one positive fixture with explicit `[covers=...]` plus concrete fixture/helper/command references.

**Rationale:** Fixtures are cheap, deterministic, and exercise the actual checker entrypoint used by the Nix rail.

## Risks / Trade-offs

- **False positives:** Generic deterministic-check wording could overlap with existing specific categories. Mitigation: only trigger on explicit deterministic-check/fixture-coverage terms and require a concrete artifact marker on a task line.
- **Guidance drift:** Mitigation: update docs and the checker's guidance/wiring self-check in the same slice.
