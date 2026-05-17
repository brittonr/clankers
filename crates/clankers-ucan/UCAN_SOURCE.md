# UCAN Source and Release Policy

`clankers-ucan` has two authorization layers while `integrate-ucan-effect-permissions` is being implemented:

1. The existing `clanker-auth` token layer remains the default build.
2. The optional `external-ucan` feature compiles the public-API adapter in `src/external_adapter.rs` against the sibling UCAN crate at `../../../ucan`.

## Current development source

The development source is the sibling checkout:

```toml
ucan = { path = "../../../ucan", optional = true }
```

That dependency is intentionally feature-gated:

```toml
external-ucan = ["dep:ucan"]
```

Default Clankers builds do not require the sibling checkout. Adapter validation must explicitly opt in:

```bash
CARGO_TARGET_DIR=$PWD/target/agent cargo check -p clankers-ucan
CARGO_TARGET_DIR=$PWD/target/agent cargo test -p clankers-ucan --features external-ucan external_adapter --no-fail-fast
```

## Public API boundary

`src/external_adapter.rs` may import only symbols exported by the root `ucan` crate. It must not import `ucan::token::*`, `ucan::verified::*`, `ucan_core::*`, or files from `../ucan/src`. If the adapter needs a missing primitive, add it to the public UCAN root API first, then consume that public export from Clankers.

## Release behavior

A release build with the default feature set is supported without `../ucan`.

A release build that enables `external-ucan` is unsupported until this dependency is converted to a reproducible source. Acceptable release sources are, in order of preference:

1. A committed workspace-local crate snapshot under `crates/` or `vendor/` with provenance metadata.
2. A pinned git dependency with an immutable `rev` recorded in `Cargo.lock` and mirrored in Nix inputs if Nix evaluates that feature.
3. A crates.io release pinned by `Cargo.lock` once UCAN is published.

Do not ship an `external-ucan` build that depends on a mutable sibling path checkout. If CI or release automation enables `external-ucan` before one of the reproducible sources above exists, the release step must fail closed and report that the UCAN source is development-only.

## Lockfile notes

`Cargo.lock` may contain `ucan`, `ucan-core`, and `verified-logic` entries after local adapter tests are run. Those entries document the local development graph but are not sufficient release provenance while the dependency source is `../../../ucan`.
