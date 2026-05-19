## ADDED Requirements

### Requirement: Provider adapter kit polish [r[embedded-composition-kits.provider-adapter-kit]]

The system MUST make product-owned model-provider adaptation easy to copy without importing Clankers provider runtime shells.

#### Scenario: Provider adapter recipe covers outcome classes [r[embedded-composition-kits.provider-adapter-kit.outcomes]]

- GIVEN a product-owned `ModelHost` adapter recipe is checked into examples
- WHEN the recipe runs
- THEN it MUST demonstrate completed, retryable-failure, terminal-failure, and usage-accounting outcomes
- THEN each outcome MUST be asserted without live credentials, network access, OAuth stores, provider discovery, or router daemon RPC

#### Scenario: Provider fixtures are explicit and hashed [r[embedded-composition-kits.provider-adapter-kit.fixtures]]

- GIVEN provider-adapter examples use request/response fixtures
- WHEN verification runs
- THEN fixture inputs and expected normalized outputs MUST be explicit literals or exported data, not produced by the code path under test
- THEN the embedded release receipt SHOULD include BLAKE3 hashes for representative request fixtures, response fixtures, and adapter-run receipts

#### Scenario: Model capability declarations stay product-owned [r[embedded-composition-kits.provider-adapter-kit.nickel-profile]]

- GIVEN a product declares model limits, retry policy, or feature flags for its adapter
- WHEN those declarations are contract checked
- THEN Nickel MAY validate the example profile shape and defaults at author time
- THEN the generic SDK MUST expose Rust traits and DTOs rather than a Nickel-dependent provider abstraction
