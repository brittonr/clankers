## ADDED Requirements

### Requirement: Root reusable policy is inventoried [r[sdk-root-brick-extraction.inventory]]

The root `clankers` crate MUST inventory tool, mode, runtime-service, slash-command, daemon, and projection modules that own reusable policy rather than product-shell wiring.

#### Scenario: root modules are classified [r[sdk-root-brick-extraction.inventory.classification]]
- GIVEN a root module depends on internal workspace crates or owns behavior used outside one CLI surface
- WHEN architecture inventory runs
- THEN the module MUST be classified as wiring, projection, adapter, or reusable policy owner
- AND reusable policy owners MUST name the target brick or explain why they remain root-local

### Requirement: Reusable root policy moves to brick owners [r[sdk-root-brick-extraction.brick-owner]]

Reusable behavior currently trapped in the root crate MUST move to workspace bricks or focused adapter modules when it can be expressed as neutral services, DTOs, or product-facing kits.

#### Scenario: extraction selects a concrete policy cluster [r[sdk-root-brick-extraction.brick-owner.selected-cluster]]
- GIVEN the root inventory identifies multiple reusable policy clusters
- WHEN a drain slice starts
- THEN it MUST select one cluster with a named owner, boundary, and parity tests
- AND the selected owner MUST be usable without launching the root CLI/TUI/daemon shell

#### Scenario: root becomes wiring-only for moved policy [r[sdk-root-brick-extraction.brick-owner.root-wiring-only]]
- GIVEN a selected policy cluster is extracted
- WHEN root code calls the moved behavior
- THEN root MUST only parse input, assemble concrete services, register tools/modes, or project output
- AND domain policy MUST live in the selected owner

### Requirement: Root extraction rails preserve ownership [r[sdk-root-brick-extraction.rails]]

Architecture rails MUST distinguish legitimate root wiring from reusable policy ownership and provide owner receipts for remaining root edges.

#### Scenario: owner receipts explain root edges [r[sdk-root-brick-extraction.rails.owner-receipts]]
- GIVEN root keeps a dependency or behavior after extraction
- WHEN the lego rail reports the edge
- THEN it MUST include the owner category, convergence condition, and target replacement path

### Requirement: Root brick extraction is behavior-preserving [r[sdk-root-brick-extraction.verification]]

Every root brick extraction MUST prove both brick-local behavior and unchanged desktop product behavior.

#### Scenario: brick tests run without root shell [r[sdk-root-brick-extraction.verification.brick-tests]]
- GIVEN extracted behavior is reusable
- WHEN focused tests run for the owner crate or adapter module
- THEN they MUST exercise the behavior without constructing root CLI, TUI, daemon, or global desktop services

#### Scenario: root parity remains unchanged [r[sdk-root-brick-extraction.verification.root-parity]]
- GIVEN root wires the extracted policy
- WHEN existing CLI/TUI/daemon paths execute the behavior
- THEN user-visible output, protocol events, and receipts MUST match the prior contract
