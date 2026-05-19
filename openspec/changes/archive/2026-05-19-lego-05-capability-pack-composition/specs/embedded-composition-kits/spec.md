## MODIFIED Requirements

### Requirement: Composable capability-pack contracts [r[embedded-composition-kits.capability-pack-composition]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Pack merge order is deterministic [r[embedded-composition-kits.capability-pack-composition.pack-merge-order-is-deterministic]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Combining multiple capability packs yields a stable ordered capability set with exact snapshot coverage.

#### Scenario: Dangerous conflicts fail closed [r[embedded-composition-kits.capability-pack-composition.dangerous-conflicts-fail-closed]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Combinations such as safe-pack plus shell/network/secret-adjacent expansion fail with typed diagnostics unless a product-owned approval policy explicitly allows the combination.

#### Scenario: Pack policy is checked before Rust use [r[embedded-composition-kits.capability-pack-composition.pack-policy-is-checked-before-rust-use]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Nickel may validate pack names, capability atoms, danger class, merge priority, and approval labels at author time, but generic SDK crates consume checked Rust data or fixtures only.
