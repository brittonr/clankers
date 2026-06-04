## ADDED Requirements

### Requirement: Embedded SDK API inventory is typed [r[embedded-composition-kits.api-inventory-typed]]

The embedded SDK API inventory rail MUST collect public SDK surface area using typed Rust/Cargo structure rather than only line-oriented string matching.

#### Scenario: inventory includes methods fields and reexports [r[embedded-composition-kits.api-inventory-typed.methods-fields-reexports]]
- GIVEN a green SDK crate exposes a public type, method, field, function, constant, module, trait, type alias, or root reexport
- WHEN the API inventory rail runs
- THEN the item MUST be inventoried or explicitly excluded with a documented owner reason
- AND the diagnostic for missing classification MUST name the source path and item owner

#### Scenario: inventory ignores test-only APIs precisely [r[embedded-composition-kits.api-inventory-typed.test-only-exclusion]]
- GIVEN SDK crates include test-only fixtures or helper APIs
- WHEN inventory collection runs
- THEN test-only items MUST be excluded without hiding later runtime items in the same source file
- AND runtime items after `cfg(test)` modules MUST still be scanned

### Requirement: Stable API classification remains deterministic [r[embedded-composition-kits.api-inventory-stability]]

Inventory classification, counts, and stable-contract hashes MUST stay deterministic and reviewable after the typed inventory expansion.

#### Scenario: stable-contract hash covers supported API only [r[embedded-composition-kits.api-inventory-stability.stable-hash]]
- GIVEN the generated inventory contains supported, optional-support, compatibility-alias, experimental, and unsupported-internal rows
- WHEN the brick inventory stability rail computes its stable-contract hash
- THEN only stable contract labels MUST contribute to migration-sensitive hashes
- AND experimental or unsupported/internal churn MUST still update total counts and receipts

#### Scenario: inventory diagnostics guide remediation [r[embedded-composition-kits.api-inventory-stability.owner-diagnostics]]
- GIVEN a public SDK item is unclassified or classified under the wrong owner
- WHEN validation fails
- THEN the diagnostic MUST explain whether to add an inventory row, hide the item, move it to an app-edge module, or update migration notes
