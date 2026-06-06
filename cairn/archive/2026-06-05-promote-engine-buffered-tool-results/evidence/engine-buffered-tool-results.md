# Engine buffered tool-results promotion evidence

Evidence-ID: promote-engine-buffered-tool-results.engine-buffered-tool-results
Artifact-Type: command-output-summary
Task-ID: I1,I2,V1
Covers: embedded-composition-kits.experimental-port-budget.engine-buffered-results-supported
Date: 2026-06-05
Status: PASS

## Implementation summary

- Promoted `clankers-engine::EngineBufferedToolResult` and its public fields from `experimental` to `supported` in `docs/src/generated/embedded-sdk-api.md`.
- Updated `policy/embedded-lego/experimental-sdk-port-budget.json` so `engine-buffered-tool-results` now expects supported rows with reducer-state validation.
- Refreshed `policy/embedded-lego/brick-inventory-stability.json` after the stable contract changed.
- Refreshed the runtime facade generated inventory because the SDK aggregate rail reported it stale during validation.

## Relevant output

```text
cargo test -p clankers-engine --lib buffered
running 0 tests
exit=0
note: this stale filter was not used as evidence.

cargo test -p clankers-engine --lib tool_feedback
running 5 tests
5 passed; 0 failed
exit=0

scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 659 public items (664 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 19 experimental rows; 141 promoted rows
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
optional-support=67
compatibility-alias=0
experimental=19
unsupported-internal=61
stable-contract=584
stable_contract_blake3=c55056c9e82b8ba6c063b6ea574114ec37cc2e694f52703fcf2bde9942c7827b
```

The only remaining experimental group in the port budget is `message-transcript-user-assistant-tool-records`, with 19 `transcript-compat` compatibility rows.
