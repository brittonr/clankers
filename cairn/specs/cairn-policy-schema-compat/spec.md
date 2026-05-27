# cairn-policy-schema-compat Specification

## Purpose

Keep Clankers' checked-in Cairn policy artifact compatible with both the repo-local pinned Cairn flake and the current external Cairn checkout used for lifecycle tooling.

## Requirements

### Requirement: Generated policy schema fields are present [r[cairn-policy-schema-compat.generated-policy-schema-fields]]
The generated Cairn policy artifact MUST include top-level policy objects required by current Cairn, including `change_metadata_policy` and `steel_orchestration_policy`.

#### Scenario: current Cairn accepts generated policy metadata [r[cairn-policy-schema-compat.generated-policy-schema-fields.change-metadata-present]]
- GIVEN `cairn-policy/generated/cairn-policy.json` is checked in
- WHEN current Cairn parses the policy during validation
- THEN the policy MUST include non-empty allowed change groups
- AND the policy MUST include accepted change statuses
- AND the policy SHOULD include any accepted group prefixes needed for feature-tagged changes

#### Scenario: current Cairn accepts generated Steel orchestration policy [r[cairn-policy-schema-compat.generated-policy-schema-fields.steel-orchestration-present]]
- GIVEN `cairn-policy/generated/cairn-policy.json` is checked in
- WHEN current Cairn parses the policy during validation
- THEN the policy MUST include a `steel_orchestration_policy` object
- AND the policy MUST include an explicit `enabled` flag
- AND the policy MUST include at least one profile with mode, deterministic budget, redaction class, fallback mode, allowed host functions, and receipt schema version fields

### Requirement: Current and pinned Cairn validation [r[cairn-policy-schema-compat.current-and-pinned-cairn-validation]]
Clankers MUST validate with both the repo-local pinned Cairn flake and the current external Cairn checkout when the policy artifact is refreshed for schema compatibility.

#### Scenario: dual validation proves schema compatibility [r[cairn-policy-schema-compat.current-and-pinned-cairn-validation.dual-validation]]
- GIVEN the policy artifact has been refreshed for current Cairn
- WHEN `nix run .#cairn -- validate --root .` runs from Clankers
- THEN validation MUST pass
- AND when `nix run path:/home/brittonr/git/cairn#cairn -- validate --root .` runs from Clankers
- THEN validation MUST pass
