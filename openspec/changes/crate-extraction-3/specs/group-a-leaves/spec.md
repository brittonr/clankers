# Group A: Remaining Leaf Extractions — Spec

## ADDED Requirements

### Requirement: nix Extraction
The `clankers-nix` crate MUST be extracted to `clanker-nix`. The snix git revision MUST be preserved as-is, and the `eval` and `refscan` feature flags MUST carry over intact.

#### Scenario: Nix crate preserves snix pin and feature contracts
- GIVEN `crates/clankers-nix/` with snix dependencies pinned at rev `8fe3bade...`
- WHEN it is extracted to the `clanker-nix` repo
- THEN `cargo check` passes with default features
- AND `cargo check --features eval` passes
- AND `cargo check --features refscan` passes
- AND store path parsing, flakeref validation, and derivation reading still work
- AND all `clankers_nix` references in source are renamed to `clanker_nix`

### Requirement: matrix Extraction
The `clankers-matrix` crate MUST be extracted to `clanker-matrix`. The matrix-sdk dependency with `e2e-encryption`, `sqlite`, and `rustls-tls` features MUST be preserved.

#### Scenario: Matrix crate preserves bridge features
- GIVEN `crates/clankers-matrix/` with matrix-sdk and ruma dependencies
- WHEN it is extracted to the `clanker-matrix` repo
- THEN the client, bridge, room, and protocol modules compile
- AND E2E encryption support is preserved
- AND markdown rendering still works
- AND all `clankers_matrix` references are renamed to `clanker_matrix`

### Requirement: zellij Extraction
The `clankers-zellij` crate MUST be extracted to `clanker-zellij`. The iroh QUIC dependency with `address-lookup-mdns` MUST stay aligned with the version used by the workspace.

#### Scenario: Zellij crate preserves iroh streaming contract
- GIVEN `crates/clankers-zellij/` with iroh and tokio runtime dependencies
- WHEN it is extracted to the `clanker-zellij` repo
- THEN P2P terminal streaming compiles
- AND the iroh mDNS discovery feature remains enabled
- AND all `clankers_zellij` references are renamed to `clanker_zellij`
