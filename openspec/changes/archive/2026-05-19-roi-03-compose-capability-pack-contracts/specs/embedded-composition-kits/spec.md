## ADDED Requirements

### Requirement: Composable capability-pack contracts [r[embedded-composition-kits.capability-pack-composition]]

The system MUST let embedders compose capability packs while preserving fail-closed safety boundaries.

#### Scenario: Capability packs merge predictably [r[embedded-composition-kits.capability-pack-composition.merge]]

- GIVEN a product selects multiple capability packs
- WHEN the packs are merged
- THEN the resulting ordered capability set MUST be deterministic and snapshot-tested
- THEN conflicts such as safe-pack plus dangerous override MUST produce typed diagnostics unless an explicit approval policy permits the combination

#### Scenario: Pack policy is schema checked before Rust use [r[embedded-composition-kits.capability-pack-composition.nickel-policy]]

- GIVEN capability packs are declared in a data-oriented policy file
- WHEN the policy is exported
- THEN Nickel contracts SHOULD validate pack names, capability atoms, danger class, merge priority, default status, and required human-approval labels
- THEN generic SDK crates MUST consume typed Rust data or generated fixtures rather than depending on Nickel at runtime

#### Scenario: Safety snapshots are content addressed [r[embedded-composition-kits.capability-pack-composition.blake3-snapshots]]

- GIVEN the acceptance rail evaluates capability-pack presets and composed packs
- WHEN it emits evidence
- THEN it MUST include BLAKE3 hashes for exported pack policy, exact allowed-capability snapshots, and dangerous-capability denial fixtures
- THEN a silent expansion of a safe pack MUST change evidence and fail focused tests unless docs and expected snapshots are updated
