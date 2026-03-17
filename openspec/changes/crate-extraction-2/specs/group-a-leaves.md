# Group A: Leaf Extractions — Spec

## Purpose

Contracts for the four leaf crates that have zero internal dependencies and
at most 1–2 reverse dependencies. These are the simplest extractions.

## Requirements

### plugin-sdk Extraction

The `clankers-plugin-sdk` crate MUST be extracted to `clanker-plugin-sdk`.
It already declares its own `[workspace]` in Cargo.toml and targets
`wasm32-unknown-unknown`. The extraction is a repo move — no API changes.

GIVEN `crates/clankers-plugin-sdk/` with `[workspace]` and `crate-type = ["rlib"]`
WHEN extracted to `clanker-plugin-sdk` repo
THEN the crate compiles with `cargo build --target wasm32-unknown-unknown`
AND the `extism-pdk` re-export still works
AND the prelude module re-exports all protocol types
AND existing plugins that depend on it can switch to the git dep with
    only a Cargo.toml path change

### nix Extraction

The `clankers-nix` crate MUST be extracted to `clanker-nix`. The snix
git dependencies MUST be preserved as-is (pinned rev). Feature flags
(`eval`, `refscan`) MUST be carried over.

GIVEN `crates/clankers-nix/` with snix deps at rev `8fe3bade...`
WHEN extracted to `clanker-nix` repo
THEN `cargo check` passes with default features
AND `cargo check --features eval` passes
AND `cargo check --features refscan` passes
AND store path parsing, flakeref validation, and derivation reading work
AND all `clankers_nix` references in source are renamed to `clanker_nix`

### matrix Extraction

The `clankers-matrix` crate MUST be extracted to `clanker-matrix`. The
matrix-sdk dependency with `e2e-encryption`, `sqlite`, `rustls-tls`
features MUST be preserved.

GIVEN `crates/clankers-matrix/` with matrix-sdk 0.9 and ruma 0.12
WHEN extracted to `clanker-matrix` repo
THEN the client, bridge, room, and protocol modules compile
AND E2E encryption support is preserved
AND markdown rendering (pulldown-cmark) works
AND all `clankers_matrix` references are renamed to `clanker_matrix`

### zellij Extraction

The `clankers-zellij` crate MUST be extracted to `clanker-zellij`. The
iroh QUIC dependency with `address-lookup-mdns` feature MUST be preserved.

GIVEN `crates/clankers-zellij/` with iroh 0.96
WHEN extracted to `clanker-zellij` repo
THEN P2P terminal streaming compiles
AND the iroh mDNS discovery feature is enabled
AND all `clankers_zellij` references are renamed to `clanker_zellij`
