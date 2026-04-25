# crate-extraction-3

## Why

`crate-extraction-2` moved the highest-leverage reusable crates out of the workspace. The remaining six crates were originally planned for standalone GitHub extraction, but the product decision is now different: keep these crates as first-class workspace crates instead of publishing separate repositories.

This change closes the extraction queue by documenting that decision, preserving the existing workspace-local crate boundaries, and keeping the local verification contracts that matter for this no-op preservation scope: workspace membership, selected feature/dependency settings, infrastructure crate ownership, and generated artifact hygiene.

## What Changes

- Do **not** create standalone GitHub repositories for `clankers-nix`, `clankers-matrix`, `clankers-zellij`, `clankers-protocol`, `clankers-db`, or `clankers-hooks`.
- Keep the six crates as independent workspace members under `crates/`.
- Preserve their current crate names and import paths unless a future change explicitly renames them in-place.
- Retain local verification expectations for this no-op preservation scope:
  - `clankers-nix`: snix pin plus `eval` / `refscan` features.
  - `clankers-matrix`: Matrix SDK feature set.
  - `clankers-zellij`: iroh/mDNS support.
  - `clankers-protocol`: local ownership of daemon/client protocol types and framing code.
  - `clankers-db`: local ownership of redb schema/table APIs.
  - `clankers-hooks`: local ownership of hook dispatch/runtime types.
- Remove external-repo-only work from scope: subtree split, GitHub creation, publishing, standalone CI badges, wrapper crates, and git-dependency migration.

## Scope

### In Scope

- Confirm the six target crates remain workspace members.
- Record the user decision to avoid separate GitHub repositories.
- Keep verification scoped to the current workspace.
- Preserve any useful preflight evidence from the original extraction analysis.
- Archive this planning change once the local-workspace decision and required verification evidence are captured.

### Out of Scope

- Creating or pushing new GitHub repositories.
- Replacing path dependencies with git dependencies.
- Thin wrapper crates for migration.
- Removing any of the six crates from the workspace.
- Renaming `clankers-*` packages to `clanker-*` in this change.
- Publishing crates or configuring standalone CI.

## Impact

- **Code:** No runtime code changes are required by this scope decision.
- **Repository layout:** The six crates stay under `crates/`.
- **OpenSpec:** Delta specs and tasks are rewritten from external extraction to workspace-local preservation.
- **Future work:** If a crate needs an in-workspace rename or API reshaping, it should get its own focused OpenSpec change.
