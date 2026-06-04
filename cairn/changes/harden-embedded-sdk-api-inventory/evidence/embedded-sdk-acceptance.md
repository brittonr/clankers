# Embedded SDK acceptance evidence

Evidence-ID: harden-embedded-sdk-api-inventory.embedded-sdk-acceptance
Artifact-Type: command-output-summary
Task-ID: V2
Covers: embedded-composition-kits.api-inventory-stability, embedded-composition-kits.api-inventory-stability.stable-hash
Date: 2026-06-04
Status: PASS

## Commands

```text
scripts/emit-embedded-sdk-release-receipt.rs
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
```

## Relevant output

```text
scripts/emit-embedded-sdk-release-receipt.rs
embedded SDK release receipt written to target/embedded-sdk-release/receipt.json
exit=0

pueue task 8: embedded-sdk-acceptance-final
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-embedded-agent-sdk.rs
Start: Thu, 4 Jun 2026 00:30:46 -0400
End:   Thu, 4 Jun 2026 00:37:25 -0400
embedded-agent-sdk acceptance passed
exit=0
```

## Receipt hash coverage

`target/embedded-sdk-release/receipt.json` includes the refreshed generated inventory and policy artifacts:

```text
docs/src/generated/embedded-sdk-api.md
  blake3=4aa60d2ab70ccb42f1e86cf237511db1dfb5ff5a49bbcf000b7b5c0a08657640
  bytes=75682

policy/embedded-lego/brick-inventory-stability.json
  blake3=d2a27c15fd8caf6d155738666d2b4f67e21d1d3e724c206d47acfe6c8358fc90
  bytes=1194
```

The policy artifact pins the typed inventory totals and stable-contract hash used by `scripts/check-brick-inventory-stability.rs`.
