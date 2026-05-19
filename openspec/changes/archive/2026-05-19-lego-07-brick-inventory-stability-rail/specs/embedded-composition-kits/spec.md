## MODIFIED Requirements

### Requirement: Embedded brick contract stability [r[embedded-composition-kits.brick-contracts]]

The system MUST keep the public brick inventory explicit, content-addressed, and tied to migration evidence so advertised green SDK entrypoints cannot drift silently.

#### Scenario: Supported brick inventory is explicit [r[embedded-composition-kits.brick-contracts.supported-brick-inventory-is-explicit]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Every documented green SDK entrypoint maps to an exported Rust item or checked example path and is classified as supported, compatibility alias, or internal/non-contract.

#### Scenario: Breaking brick drift requires migration evidence [r[embedded-composition-kits.brick-contracts.breaking-brick-drift-requires-migration-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Removing, renaming, or semantically repurposing a supported brick fails verification until docs, migration notes, examples, and receipt evidence are updated together.

#### Scenario: Boundary policy stays in generated evidence [r[embedded-composition-kits.brick-contracts.boundary-policy-stays-in-generated-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The release receipt includes hashes and byte sizes for API inventory, docs, checker policy, and examples so downstream embedders can audit the exact brick contract they consumed.
