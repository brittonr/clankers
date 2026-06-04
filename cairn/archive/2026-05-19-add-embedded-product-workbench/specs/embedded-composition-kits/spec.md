## ADDED Requirements

### Requirement: Product-workbench embedded dogfood [r[embedded-composition-kits.product-workbench]]

The system MUST provide a checked product-style embedded dogfood recipe that composes product-owned session storage, product-owned provider adaptation, and product-owned tool execution in one in-process integration while preserving generic SDK boundaries.

#### Scenario: Combined seams run through green SDK crates [r[embedded-composition-kits.product-workbench.combined-seams]]

- GIVEN a product-style workbench example composes an embedded agent from documented SDK crates
- WHEN the example runs its first turn and a restored follow-up turn
- THEN it MUST route model execution through a product-owned `ModelHost` adapter, product tools through `EmbeddedToolCatalog`/`CatalogToolExecutor`, and persistence through product-owned session/message/receipt DTOs
- THEN it MUST NOT import Clankers daemon sockets, TUI/rendering crates, provider discovery, OAuth stores, Clankers DB/session ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles

#### Scenario: Product-workbench persists and restores context [r[embedded-composition-kits.product-workbench.example]]

- GIVEN the product-workbench example runs an initial tool-using turn
- WHEN it persists the resulting transcript and reloads the same product-owned session for a follow-up prompt
- THEN the follow-up model request MUST include the prior user/tool/assistant context and the new prompt in deterministic order
- THEN the example MUST persist a product-owned turn receipt that records session id, turn index, model request count, tool call summaries, and usage totals

#### Scenario: Product-workbench fails closed [r[embedded-composition-kits.product-workbench.fail-closed]]

- GIVEN the product-workbench example receives a missing session id or a catalog entry requiring dangerous capabilities without approval
- WHEN the recipe attempts to run that path
- THEN missing-session handling MUST return an explicit product-owned error before model/tool execution and MUST NOT create a replacement session
- THEN dangerous-tool handling MUST deny execution before product tool code runs and MUST expose the denial as deterministic recipe evidence
