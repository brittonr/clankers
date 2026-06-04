## MODIFIED Requirements

### Requirement: Real product dogfood integration [r[embedded-composition-kits.real-product-dogfood]]

The system MUST extend this requirement with the next lego-readiness slice.

#### Scenario: Product dogfood manifest is checked [r[embedded-composition-kits.real-product-dogfood.product-dogfood-manifest-is-checked]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN A selected product embedding declares its SDK crate set, capability packs, tool catalog references, provider/session seams, and shell exclusions in a checked manifest before runtime evidence is accepted.

#### Scenario: Dogfood run emits reproducible transcript evidence [r[embedded-composition-kits.real-product-dogfood.dogfood-run-emits-reproducible-transcript-evidence]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN The dogfood rail emits a sanitized transcript plus dependency-boundary report and BLAKE3 receipt without live credentials, network access, daemon startup, provider discovery, or user-local state.

#### Scenario: Dogfood findings drive brick backlog [r[embedded-composition-kits.real-product-dogfood.dogfood-findings-drive-brick-backlog]]

- GIVEN the embedded SDK lego-readiness rail is being advanced

- WHEN this slice is implemented and verified

- THEN If the dogfood integration needs app-owned glue that appears reusable across products, the result is recorded as a follow-up OpenSpec rather than silently expanding green SDK dependencies.
