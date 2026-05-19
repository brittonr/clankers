## ADDED Requirements

### Requirement: Real product dogfood integration [r[embedded-composition-kits.real-product-dogfood]]

The system MUST prove lego-style Clankers composition in at least one real product integration before promoting more generic SDK API.

#### Scenario: Product consumes only green SDK bricks [r[embedded-composition-kits.real-product-dogfood.green-surface]]

- GIVEN a selected product integrates an embedded Clankers agent
- WHEN its dependency graph and source imports are checked
- THEN it MUST use only documented green generic SDK crates for in-process engine composition
- THEN it MUST NOT import daemon sockets, TUI/rendering crates, provider discovery, OAuth stores, Clankers session DB ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles

#### Scenario: Dogfood evidence is content addressed [r[embedded-composition-kits.real-product-dogfood.receipt]]

- GIVEN the product dogfood rail completes
- WHEN it emits a receipt
- THEN the receipt MUST include BLAKE3 hashes for the dogfood manifest, dependency-boundary report, executable recipe or integration test source, and sanitized runtime transcript
- THEN changing any hashed evidence artifact MUST change the receipt without requiring live credentials or network access

#### Scenario: Product manifest is contract checked [r[embedded-composition-kits.real-product-dogfood.nickel-manifest]]

- GIVEN the integration declares its embedded-agent composition in a checked manifest
- WHEN the manifest is exported for the dogfood rail
- THEN Nickel contracts SHOULD validate selected crates, capability packs, tool catalog references, and forbidden shell surfaces before Rust tests run
- THEN the Rust runtime MUST consume exported typed data or generated fixtures rather than evaluating Nickel inside generic SDK crates
