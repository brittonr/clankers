## Context

Current Cairn added a required top-level `change_metadata_policy` field to the policy contract. Clankers tracks only the generated JSON artifact under `cairn-policy/generated/`, so the compatibility repair is a narrow artifact refresh rather than a source-code change.

## Decisions

### 1. Keep the policy refresh data-only

**Choice:** Add the current Cairn `change_metadata_policy` object to the generated policy artifact with the default allowed groups, group prefix, and statuses.

**Rationale:** The field is policy metadata consumed by Cairn validation. Adding it as a top-level JSON object preserves existing policy content while satisfying the current schema.

### 2. Verify both Cairn entrypoints

**Choice:** Treat repo-local pinned Cairn validation and current external Cairn validation as the acceptance rail for this change.

**Rationale:** The bug is cross-version schema drift. Either validation passing alone would miss the regression class.

## Risks / Trade-offs

- The generated policy has no checked-in Nickel source in Clankers, so this change updates the artifact directly.
- If old Cairn rejected unknown fields this would require a lockstep flake update; verification keeps that risk explicit.
