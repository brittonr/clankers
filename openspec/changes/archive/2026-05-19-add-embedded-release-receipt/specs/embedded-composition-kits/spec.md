## ADDED Requirements

### Requirement: Product embedding release receipts [r[embedded-composition-kits.acceptance-rail.release-receipt]]

The system MUST provide a deterministic release receipt for product embedders that captures the embedded SDK readiness boundary and the artifacts used as evidence.

#### Scenario: Receipt records verifiable SDK evidence [r[embedded-composition-kits.acceptance-rail.release-receipt.artifacts]]

- GIVEN a developer runs the embedded SDK acceptance rail or the receipt helper directly
- WHEN the receipt is generated
- THEN it MUST include the current commit identifier, commit date when available, and `git status --short --branch` output
- THEN it MUST include BLAKE3 hashes and byte sizes for the embedded SDK guide, generated API inventory, canonical embedded composition spec, acceptance/check scripts, and standalone embedded examples
- THEN it MUST include the maintained verification commands needed before claiming product embedding readiness

#### Scenario: Receipt preserves green/yellow/red boundaries [r[embedded-composition-kits.acceptance-rail.release-receipt.boundaries]]

- GIVEN a product team reviews the generated receipt before embedding Clankers
- WHEN it inspects the SDK boundary fields
- THEN the receipt MUST identify the green generic SDK crates, yellow app-edge integration surfaces, and red shell/runtime exclusions
- THEN the receipt MUST NOT present daemon, TUI, provider discovery, OAuth stores, session database ownership, Matrix, iroh/P2P, plugin supervision, or built-in tool bundles as generic embedded SDK dependencies

#### Scenario: Acceptance rail emits receipt evidence [r[embedded-composition-kits.acceptance-rail.release-receipt.one-command]]

- GIVEN `scripts/check-embedded-agent-sdk.sh` is the maintained one-command lego readiness rail
- WHEN the command succeeds
- THEN it MUST run the receipt helper and leave a machine-readable receipt under a deterministic target-directory path
- THEN receipt generation MUST NOT add runtime dependencies to the reusable SDK crates or require live credentials, network access, daemon startup, provider discovery, OAuth stores, TUI setup, or Clankers session database access
