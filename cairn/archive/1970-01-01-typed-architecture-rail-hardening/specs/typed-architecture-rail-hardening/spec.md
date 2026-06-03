## ADDED Requirements

### Requirement: Brittle source anchors are inventoried [r[typed-architecture-rail-hardening.anchor-inventory]]

Architecture rails MUST track exact string anchors by ownership concern and document why each remaining exact anchor is necessary.

#### Scenario: selected cluster has an anchor inventory [r[typed-architecture-rail-hardening.anchor-inventory.selected-cluster]]
- GIVEN a rail cluster is selected for hardening
- WHEN the inventory is reviewed
- THEN every exact string anchor in that cluster MUST be classified as replaceable, behavior-owned, or intentionally exact

### Requirement: Ownership checks become typed or behavioral [r[typed-architecture-rail-hardening.typed-checks]]

Selected architecture rail clusters SHOULD use Rust AST inventories, Cargo metadata, generated ownership manifests, or behavior fixtures instead of exact source-string presence checks when practical.

#### Scenario: refactor-preserving movement stays green [r[typed-architecture-rail-hardening.typed-checks.refactor-safe]]
- GIVEN code moves without changing ownership
- WHEN the hardened rail runs
- THEN it SHOULD remain green if the typed owner contract is preserved
- AND it MUST still fail if ownership moves to the wrong layer

### Requirement: Rail diagnostics name owners [r[typed-architecture-rail-hardening.diagnostics]]

Architecture rail failures MUST name the source, target owner, and expected replacement path for the selected cluster.

#### Scenario: diagnostic avoids grep archaeology [r[typed-architecture-rail-hardening.diagnostics.owner-path]]
- GIVEN a hardened rail fails
- WHEN a developer reads the diagnostic
- THEN it MUST identify what owner was expected and where the behavior should live

### Requirement: Rail hardening validation passes [r[typed-architecture-rail-hardening.verification]]

Rail hardening MUST preserve the existing ownership guarantee and pass focused validation.

#### Scenario: hardened rail catches a negative path [r[typed-architecture-rail-hardening.verification.negative-path]]
- GIVEN the selected ownership concern regresses
- WHEN the hardened rail or fixture runs
- THEN it MUST fail for the regression without relying on formatting-specific strings
