# Transcript compatibility message-record promotion evidence

Evidence-ID: promote-transcript-compat-message-records.transcript-compat-message-records
Artifact-Type: command-output-summary
Task-ID: I1,I2,I3,V1
Covers: embedded-composition-kits.experimental-port-budget.transcript-compat-records-supported
Date: 2026-06-05
Status: PASS

## Implementation summary

- Promoted `UserMessage`, `AssistantMessage`, `ToolResultMessage`, and their public fields from `experimental` to `optional-support` in `docs/src/generated/embedded-sdk-api.md`.
- Updated `policy/embedded-lego/experimental-sdk-port-budget.json` so the transcript compatibility group is promoted with non-default feature and message-boundary validation evidence.
- Updated `scripts/check-experimental-sdk-port-budget.rs` so promoted budget groups may require `supported`, `optional-support`, or `compatibility-alias` stability labels.
- Updated `scripts/check-message-contract-boundary.rs` so its inventory expectations match the optional-support transcript compatibility records while keeping transcript APIs out of the default green surface.
- Refreshed `policy/embedded-lego/brick-inventory-stability.json` after the stable contract changed.

## Relevant output

```text
cargo test -p clanker-message --features transcript-compat
running 28 tests
28 passed; 0 failed
Doc-tests: 0 passed; 0 failed
exit=0

scripts/check-message-contract-boundary.rs
ok: message contract boundary rail passed
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 659 public items (664 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 0 experimental rows; 160 promoted rows
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0
```

## Inventory summary

`policy/embedded-lego/brick-inventory-stability.json` now pins:

```text
total=664
supported=517
optional-support=86
compatibility-alias=0
experimental=0
unsupported-internal=61
stable-contract=603
stable_contract_blake3=c5869e1222c439151f4e26c7ed1b748d375bb5797755d4d927c6287af525a4c2
```

The experimental SDK port budget now expects zero `experimental` rows; all remaining public transcript compatibility records are optional support behind the `transcript-compat` feature.
