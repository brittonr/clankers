# ADDED Requirements

### Requirement: lineage audit [r[readiness-tag-lineage-audit.tag-targets]]

The lineage audit MUST identify each nearby readiness tag and target commit.

#### Scenario: Tag targets are visible
- GIVEN operators inspect readiness docs
- WHEN the lineage audit is present
- THEN it lists the relevant readiness tags and exact target commits

### Requirement: lineage audit [r[readiness-tag-lineage-audit.evidence-boundary]]

The lineage audit MUST distinguish tag evidence from later commits or docs-only evidence.

#### Scenario: Evidence boundary is clear
- GIVEN a later commit exists after a readiness tag
- WHEN operators read the lineage
- THEN the docs do not imply the older tag covers the later commit

### Requirement: lineage audit [r[readiness-tag-lineage-audit.no-tag-move]]

The lineage audit MUST NOT move existing tags.

#### Scenario: Audit is non-mutating
- GIVEN the lineage audit is implemented
- WHEN git tags are inspected after the change
- THEN existing tag names still point to their original commits unless a separate explicit tag task is requested

### Requirement: audit [r[readiness-tag-lineage-audit.lineage-check]]

The audit MUST have a focused verification check for tag names or documented targets.

#### Scenario: Lineage check catches stale docs
- GIVEN a documented tag target drifts
- WHEN the focused check runs
- THEN it reports a deterministic mismatch
