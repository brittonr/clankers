## MODIFIED Requirements

### Requirement: Architecture rails become typed or behavioral [r[remaining-coupling-drain.architecture-rail-hardening]]

Architecture boundary verification MUST replace brittle string-presence anchors with typed Cargo metadata, Rust AST/module inventories, deterministic behavior fixtures, or generated ownership manifests whenever practical.

#### Scenario: workspace layer direction is generated [r[remaining-coupling-drain.architecture-rail-hardening.workspace-layer-map]]
- GIVEN Clankers crates are assigned to green contract, host/facade, orchestration, and application-shell layers
- WHEN workspace dependency and constructor inventories are generated
- THEN lower layers MUST NOT depend on or construct higher-layer types except through documented adapter seams
- AND rail diagnostics MUST name the source owner, forbidden target layer, and expected replacement path
