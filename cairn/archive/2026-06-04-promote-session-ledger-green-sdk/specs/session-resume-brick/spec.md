## ADDED Requirements

### Requirement: Session ledger core is a green SDK boundary [r[session-resume-brick.green-ledger-core]]

The reusable session ledger DTOs and deterministic replay helpers MUST live in a green SDK owner or equivalently dependency-clean green module that can be used by minimal embedding products without importing `clankers-runtime`, `clankers-session`, desktop storage, daemon protocol, database, or TUI crates.

#### Scenario: ledger core excludes runtime shell concerns [r[session-resume-brick.green-ledger-core.no-runtime-shell]]
- GIVEN an embedding host persists model-visible session history
- WHEN it depends on the reusable ledger core
- THEN the dependency graph MUST exclude runtime facade, desktop session storage, daemon protocol, database, and TUI crates
- AND ledger construction MUST NOT require wall-clock timestamps, global paths, or runtime-specific error types

#### Scenario: replay remains deterministic [r[session-resume-brick.green-ledger-core.deterministic-replay]]
- GIVEN a ledger contains ordered user, assistant, tool, summary, usage, receipt, or unsupported entries
- WHEN replay projection runs
- THEN model-visible messages MUST be emitted deterministically in causal order
- AND unsupported entries MUST fail closed with a neutral error that adapters can project safely

### Requirement: Runtime and desktop session paths adapt to the green ledger [r[session-resume-brick.ledger-adapters]]

Runtime facade, desktop session storage, daemon resume seed handling, and product examples MUST consume the green ledger owner through explicit adapters rather than duplicating ledger semantics or promoting desktop storage into the SDK.

#### Scenario: product examples use reusable ledger history [r[session-resume-brick.ledger-adapters.product-examples]]
- GIVEN embedded session examples store and reload prior conversation state
- WHEN they build a follow-up model request
- THEN their persisted model-visible history SHOULD use the reusable ledger API
- AND their product-owned storage wrappers MAY remain local to the examples

#### Scenario: desktop compatibility stays app-edge [r[session-resume-brick.ledger-adapters.desktop-edge]]
- GIVEN desktop session files or daemon resume paths need Clankers transcript compatibility records
- WHEN those records cross into reusable replay logic
- THEN a named app-edge adapter MUST convert them to green ledger entries first
- AND desktop storage types MUST NOT become required by the ledger core
