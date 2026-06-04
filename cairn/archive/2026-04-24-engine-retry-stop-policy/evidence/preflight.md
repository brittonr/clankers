Artifact-Type: preflight-note
Evidence-ID: engine-retry-stop-policy.preflight
Task-ID: proposal-preflight
Covers: archive cleanup and duplicate active-change check

## Archive cleanup

Command run before finalizing this change:

```text
openspec archive -y engine-turn-migration
Task status: ✓ Complete

Specs to update:
  embeddable-agent-engine: update
Applying changes to openspec/specs/embeddable-agent-engine/spec.md:
  + 4 added
Totals: + 4, ~ 0, - 0, → 0
Specs updated successfully.
Change 'engine-turn-migration' archived as '2026-04-24-engine-turn-migration'.
```

## Active change check

`openspec list` after archive shows `engine-retry-stop-policy` active and no active `engine-turn-migration` entry. The archived change now lives at `openspec/changes/archive/2026-04-24-engine-turn-migration/`.

## Canonical spec check

`openspec/specs/embeddable-agent-engine/spec.md` now includes the archived first executable engine-slice requirements. This change builds on those requirements rather than duplicating the completed active change.
