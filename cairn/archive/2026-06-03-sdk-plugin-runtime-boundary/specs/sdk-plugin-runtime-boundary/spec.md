## ADDED Requirements

### Requirement: Plugin responsibilities are separated [r[sdk-plugin-runtime-boundary.inventory]]

Plugin manifest schema, runtime dispatch, sandbox/launch policy, tool registration, supervision, hooks, host events, and UI projection MUST be inventoried as separate responsibilities with explicit owners.

#### Scenario: inventory names responsibility owners [r[sdk-plugin-runtime-boundary.inventory.owners]]
- GIVEN plugin code handles manifests, runtimes, sandboxing, tools, supervision, hooks, events, or UI
- WHEN architecture inventory runs
- THEN each responsibility MUST have one owner and a neutral or app-edge classification
- AND desktop-only responsibilities MUST NOT be advertised as generic SDK dependencies

### Requirement: Neutral plugin contracts avoid display leakage [r[sdk-plugin-runtime-boundary.neutral-contracts]]

Plugin manifest and tool runtime contracts intended for reuse MUST use neutral DTOs and MUST NOT depend on TUI/protocol display types.

#### Scenario: neutral contracts have no display DTOs [r[sdk-plugin-runtime-boundary.neutral-contracts.no-display-dtos]]
- GIVEN manifest validation or runtime dispatch modules are reusable
- WHEN source-boundary rails inspect them
- THEN they MUST NOT import `clanker-tui-types`, daemon protocol DTOs, or root display state
- AND UI/status projection MUST be owned by a display adapter

#### Scenario: UI projection stays at edge [r[sdk-plugin-runtime-boundary.neutral-contracts.ui-edge]]
- GIVEN a plugin emits UI, display, notification, or status information
- WHEN desktop Clankers presents it
- THEN neutral plugin output MUST be projected to TUI/protocol DTOs only at the desktop display or daemon edge

### Requirement: Runtime dispatch has one owner per runtime kind [r[sdk-plugin-runtime-boundary.dispatch]]

Extism, stdio, built-in, and product-owned plugin/tool runtime entries MUST dispatch through their owning loader and fail closed when routed to a forbidden loader.

#### Scenario: runtime owners are separate [r[sdk-plugin-runtime-boundary.dispatch.separate-owners]]
- GIVEN a plugin/tool manifest declares a runtime kind
- WHEN dispatch begins
- THEN Extism entries MUST use the WASM loader, stdio entries MUST use the stdio runtime, built-in entries MUST use the app-edge registry, and product-owned entries MUST use the product executor
- AND any other loader path MUST fail closed before execution

### Requirement: Plugin boundary split is verified [r[sdk-plugin-runtime-boundary.verification]]

Verification MUST include runtime dispatch matrix fixtures, boundary rails, and focused runtime tests.

#### Scenario: dispatch matrix covers forbidden loaders [r[sdk-plugin-runtime-boundary.verification.dispatch-matrix]]
- GIVEN runtime dispatch fixtures run
- WHEN valid and invalid runtime kinds are evaluated
- THEN each valid kind MUST route to one owner and each forbidden-loader case MUST fail closed with a safe diagnostic

#### Scenario: boundary rails reject display imports [r[sdk-plugin-runtime-boundary.verification.boundary-rails]]
- GIVEN neutral plugin modules import TUI/protocol/root display types
- WHEN validation runs
- THEN the rail MUST fail with the offending path, expected projection owner, and requirement id
