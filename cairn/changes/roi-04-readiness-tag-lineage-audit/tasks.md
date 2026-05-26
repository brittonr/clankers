# Tasks

- [ ] [serial] T1. Collect readiness tag names, target commits, and current harness evidence boundaries. [covers=r[readiness-tag-lineage-audit.tag-targets],r[readiness-tag-lineage-audit.evidence-boundary]]
- [ ] [parallel] T2. Update release-readiness docs with a compact lineage table or equivalent text. [covers=r[readiness-tag-lineage-audit.tag-targets],r[readiness-tag-lineage-audit.evidence-boundary],r[readiness-tag-lineage-audit.no-tag-move]]
- [ ] [parallel] T3. Add or update a focused docs contract check for tag-lineage facts. [covers=r[readiness-tag-lineage-audit.lineage-check]]
- [ ] [serial] T4. Verify tag targets, docs build or focused tests, and `git diff --check` without moving tags. [covers=r[readiness-tag-lineage-audit.no-tag-move],r[readiness-tag-lineage-audit.lineage-check]]
