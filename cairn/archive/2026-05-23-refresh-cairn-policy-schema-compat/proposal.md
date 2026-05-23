## Why

Clankers' checked-in Cairn policy artifact still matched the repo-local pinned Cairn schema, but current Cairn requires an explicit `change_metadata_policy` object. That drift made `nix run path:/home/brittonr/git/cairn#cairn -- validate --root .` fail even though `nix run .#cairn -- validate --root .` passed.

## What Changes

- Refresh `cairn-policy/generated/cairn-policy.json` with the current Cairn `change_metadata_policy` contract.
- Add a lifecycle requirement that Clankers' generated policy remains consumable by both the repo-local pinned Cairn and the current external Cairn checkout during schema-transition windows.

## Impact

- **Files**: `cairn-policy/generated/cairn-policy.json`, Cairn change/spec metadata.
- **Testing**: validate with both `nix run .#cairn -- validate --root .` and `nix run path:/home/brittonr/git/cairn#cairn -- validate --root .`.
