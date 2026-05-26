# Change: Readiness tag lineage audit

## Why

Clankers now has several adjacent readiness tags and commits (`05-25`, `05-26`, and `05-26-dogfood-full`). A small lineage audit can prevent stale-tag confusion in docs and release notes.

## What Changes

- Audit docs/help text for readiness tag lineage and stale current-head wording.
- Add a compact lineage table or update release-readiness docs with tag targets and evidence boundaries.
- Add a focused test or grep rail if the repo already validates readiness docs.

## Non-Goals

- No tag movement.
- No new harness run unless existing evidence is stale.
- No broad release-process redesign.
