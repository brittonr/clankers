# Message boundary rail evidence

Evidence-ID: sdk-message-contract-boundary-boundary-rails
Artifact-Type: command-output-summary
Task-ID: V2
Covers: sdk-message-contract-boundary.verification.boundary-rails,sdk-message-contract-boundary.inventory,sdk-message-contract-boundary.stable-subset.contracts,sdk-message-contract-boundary.transcript-internals.compatibility-only,sdk-message-contract-boundary.transcript-internals.edge-owned
Date: 2026-06-03
Status: PASS

## Commands

```text
scripts/check-message-contract-boundary.rs
scripts/check-embedded-sdk-api.rs
scripts/check-brick-inventory-stability.rs
scripts/check-behavioral-lego-rails.rs
scripts/check-embedded-sdk-deps.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clankers-provider -p clankers-controller -p clankers-session
```

## Relevant output

```text
scripts/check-message-contract-boundary.rs
ok: message contract boundary rail passed

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 184 public items (189 rows)

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json

scripts/check-behavioral-lego-rails.rs
behavioral lego rail inventory receipt written to target/embedded-sdk-release/behavioral-rail-inventory-receipt.json

scripts/check-embedded-sdk-deps.rs
ok: embedded SDK example dependency graph has 180 packages and excludes forbidden runtime crates

cargo check --tests -p clankers-provider -p clankers-controller -p clankers-session
Finished `dev` profile [optimized + debuginfo] target(s) in 9.58s
```

## Coverage notes

The new boundary rail requires `content` and `transcript` modules, keeps legacy `message::*` as an unsupported/internal compatibility import path, verifies stable content/contracts/streaming/semantic-event inventory labels, verifies transcript record labels/sources, rejects transcript-internal tokens in embedded examples, and rejects transcript-internal tokens in public green SDK API declarations for engine, engine-host, tool-host, and adapters. The behavioral rail inventory also wires the new boundary checker into embedded SDK acceptance. The provider/controller/session cargo check proves existing app-edge transcript adapters still type-check through compatibility exports.
