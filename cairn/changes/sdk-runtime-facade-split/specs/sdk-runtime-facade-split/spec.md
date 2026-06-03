## ADDED Requirements

### Requirement: Runtime facade surfaces are classified [r[sdk-runtime-facade-split.inventory]]

`clankers-runtime` MUST classify each public module/type as a green SDK kit, yellow app-edge service, or red desktop compatibility surface before it is advertised to embedders.

#### Scenario: support labels cover public runtime API [r[sdk-runtime-facade-split.inventory.support-labels]]
- GIVEN runtime public API inventory is generated
- WHEN a public module or re-export appears
- THEN it MUST have a support label and source owner
- AND unsupported or compatibility-only items MUST NOT be documented as stable generic SDK entrypoints

### Requirement: Runtime kits are independently consumable [r[sdk-runtime-facade-split.kits]]

Runtime capabilities MUST be split, isolated, or feature-gated so products can consume one coherent kit without unrelated provider/router/auth/plugin/TUI/daemon/process/Steel surfaces.

#### Scenario: selected kit has explicit boundary [r[sdk-runtime-facade-split.kits.selected-kit]]
- GIVEN a runtime split slice starts
- WHEN a kit is selected
- THEN the change MUST define the kit's public types, dependency graph, defaults, and migration notes
- AND the kit MUST name which runtime surfaces are out of scope

#### Scenario: consumers can import the kit alone [r[sdk-runtime-facade-split.kits.independent-consumption]]
- GIVEN an example or product fixture depends on the selected kit
- WHEN it builds and runs
- THEN unrelated runtime surfaces MUST NOT appear in its dependency graph unless explicitly allowed by the kit policy

### Requirement: Runtime services fail closed by default [r[sdk-runtime-facade-split.fail-closed]]

Runtime service defaults MUST fail closed instead of discovering desktop globals, credentials, plugins, filesystems, or network services implicitly.

#### Scenario: missing services are explicit [r[sdk-runtime-facade-split.verification.fail-closed]]
- GIVEN a runtime kit requires a host service
- WHEN the service is not injected or the policy disables discovery
- THEN the kit MUST return an unavailable/unsupported error or safe receipt
- AND it MUST NOT probe global Clankers paths, auth stores, plugin roots, or network endpoints

### Requirement: Runtime split is verified [r[sdk-runtime-facade-split.verification]]

Runtime kit changes MUST be verified by public API inventory, dependency checks, fail-closed tests, examples, and Cairn validation.

#### Scenario: dependency checks protect kit scope [r[sdk-runtime-facade-split.verification.dependency-checks]]
- GIVEN a selected runtime kit is inspected
- WHEN dependency validation runs
- THEN forbidden unrelated crates and feature surfaces MUST be absent or explicitly justified by the kit policy
