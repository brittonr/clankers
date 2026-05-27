## Context

`review_metrics action=promotions last=50 min=2` reports `omission|spec|prompt` as the largest current repeated category. Existing review-gate rails already cover broad deterministic-task omissions, oracle checkpoints, design-specific omissions, provider-status spec omissions, and deterministic-check artifact tasks. They do not yet cover the current shape where a proposal or design uses strong lifecycle language, but the delta spec drops or weakens that obligation.

The selected category should be handled as a project-local fixture-backed diagnostic first, following `review-metrics-regression-rail.project-local-first`.

## Decisions

### 1. Add a strong-constraint spec diagnostic

**Choice:** Add `missing-strong-constraint-spec` for cases where proposal/design text states a strong lifecycle constraint and the delta spec omits or weakens it.

**Rationale:** The repeated examples share one traceability failure: the implementation task ledger may look complete, but the accepted spec does not preserve the promise reviewers were asked to enforce.

**Alternative:** Add one diagnostic per example family. Rejected for this first slice because generated artifact hygiene, local verification, no-GitHub constraints, source preservation, and capability-boundary preservation are all the same artifact-shape problem.

### 2. Use paired sanitized fixtures

**Choice:** Add one negative fixture that omits or weakens strong proposal constraints and one positive fixture that preserves them with normative delta spec text.

**Rationale:** Fixtures are deterministic, safe to commit, and exercise the same checker entrypoint used by maintainers.

### 3. Keep the implementation project-local

**Choice:** Update `scripts/check-openspec-review-gates.rs`, fixtures, and Clankers authoring guidance first. Do not patch generic Cairn core in this change.

**Rationale:** Clankers has the metrics evidence and a repo-local drift rail. Generic lifecycle engine changes should wait until this rule shape proves stable.

## Risks / Trade-offs

- **False positives:** Strong words can be generic. Mitigation: fixture the trigger around lifecycle constraint families and require the diagnostic to name the source artifact plus family.
- **Over-broad scope:** Proposal/design/spec traceability can become a full semantic parser. Mitigation: start with sanitized repeated examples and deterministic text-family matching only.
- **Guidance drift:** Update operator docs with the diagnostic and required author shape in the same implementation slice.
