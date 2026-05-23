# cairn-policy-schema-compat Specification

## Purpose

Keep Clankers' checked-in Cairn policy artifact compatible with both the repo-local pinned Cairn flake and the current external Cairn checkout used for lifecycle tooling.

## Requirements

### Requirement: Change metadata policy is present [r[cairn-policy-schema-compat.change-metadata-policy]]
The generated Cairn policy artifact MUST include a top-level `change_metadata_policy` object when current Cairn requires it.

#### Scenario: current Cairn accepts generated policy metadata [r[cairn-policy-schema-compat.change-metadata-policy.present]]
- GIVEN `cairn-policy/generated/cairn-policy.json` is checked in
- WHEN current Cairn parses the policy during validation
- THEN the policy MUST include non-empty allowed change groups
- AND the policy MUST include accepted change statuses
- AND the policy SHOULD include any accepted group prefixes needed for feature-tagged changes

### Requirement: Current and pinned Cairn validation [r[cairn-policy-schema-compat.current-and-pinned-cairn-validation]]
Clankers MUST validate with both the repo-local pinned Cairn flake and the current external Cairn checkout when the policy artifact is refreshed for schema compatibility.

#### Scenario: dual validation proves schema compatibility [r[cairn-policy-schema-compat.current-and-pinned-cairn-validation.dual-validation]]
- GIVEN the policy artifact has been refreshed for current Cairn
- WHEN `nix run .#cairn -- validate --root .` runs from Clankers
- THEN validation MUST pass
- AND when `nix run path:/home/brittonr/git/cairn#cairn -- validate --root .` runs from Clankers
- THEN validation MUST pass
