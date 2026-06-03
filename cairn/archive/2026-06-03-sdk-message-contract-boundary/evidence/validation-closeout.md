# Message contract closeout validation evidence

Evidence-ID: sdk-message-contract-boundary-closeout
Artifact-Type: command-output-summary
Task-ID: V3
Covers: sdk-message-contract-boundary.verification,sdk-message-contract-boundary.verification.compat-fixtures,sdk-message-contract-boundary.verification.boundary-rails
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clanker-message
scripts/check-message-contract-boundary.rs
scripts/check-embedded-sdk-api.rs
scripts/check-brick-inventory-stability.rs
scripts/check-behavioral-lego-rails.rs
scripts/check-embedded-sdk-deps.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clankers-provider -p clankers-controller -p clankers-session
RUSTC_WRAPPER= rustfmt --check --config skip_children=true crates/clanker-message/src/content.rs crates/clanker-message/src/transcript.rs crates/clanker-message/src/message.rs crates/clanker-message/src/streaming.rs crates/clanker-message/src/lib.rs scripts/check-message-contract-boundary.rs scripts/check-embedded-agent-sdk.rs scripts/emit-embedded-sdk-release-receipt.rs
```

## Relevant output

```text
cargo nextest run -p clanker-message
Summary: 28 tests run: 28 passed, 0 skipped

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

rustfmt --check ...
exit 0
```

## Coverage notes

The closeout bundle covers the message crate module split, transcript serialization compatibility, green SDK API inventory labels, message boundary source rail and acceptance inventory wiring, embedded example dependency exclusions, provider/controller/session compatibility adapters, and touched-file formatting checks for `sdk-message-contract-boundary`.
