## MODIFIED Requirements

### Requirement: Provider adapter kit polish [r[embedded-composition-kits.provider-adapter-kit]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Provider adapter template is fixture backed [r[embedded-composition-kits.provider-adapter-kit.provider-adapter-template-is-fixture-backed]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The provider-adapter kit includes explicit request and response fixtures for completed, retryable, terminal, and usage-accounting paths rather than deriving expected fixtures from the implementation under test.

#### Scenario: Model capability profile remains product-owned [r[embedded-composition-kits.provider-adapter-kit.model-capability-profile-remains-product-owned]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN Optional model limits, retry policy, and feature flags are declared as product-owned data and consumed as typed Rust inputs, with Nickel allowed only as an author-time checker.

#### Scenario: Template dependency boundary is enforced [r[embedded-composition-kits.provider-adapter-kit.template-dependency-boundary-is-enforced]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The template and examples reject clankers-provider, clanker-router daemon RPC, OAuth stores, provider discovery, and live network credentials from the generic SDK path.
