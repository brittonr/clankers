# Implementation boundary evidence

Evidence-ID: split-message-transcript-sdk-defaults.implementation-boundary
Artifact-Type: command-output-summary
Task-ID: I1,I2,I3,I4
Covers: sdk-message-contract-boundary.default-subset, sdk-message-contract-boundary.default-subset.minimal-graph, sdk-message-contract-boundary.default-subset.root-exports, sdk-message-contract-boundary.transcript-compat-feature, sdk-message-contract-boundary.transcript-compat-feature.opt-in
Date: 2026-06-04
Status: PASS

## Implementation summary

- `clanker-message` now has `default = []` and `transcript-compat = ["dep:chrono", "dep:hex", "dep:rand"]`.
- `chrono`, `hex`, and `rand` are optional dependencies instead of default SDK dependencies.
- `clanker-message::message` and `clanker-message::transcript` are gated with `#[cfg(feature = "transcript-compat")]`.
- Crate-root transcript reexports for `AgentMessage`, `MessageId`, `generate_id`, and persisted transcript record variants were removed.
- Desktop/session/provider/controller/root adapters that need persisted transcript records enable `transcript-compat` and import through `clanker_message::transcript::...`.
- The embedded SDK guide documents `transcript-compat` as a non-default Clankers desktop/session compatibility path.

## Import inventory

`scripts/check-message-contract-boundary.rs` inventories the active production/test trees (`crates`, `src`, `tests`) and fails on root transcript import tokens such as:

```text
clanker_message::AgentMessage
clanker_message::MessageId
clanker_message::generate_id
clanker_message::message::
use clanker_message::*;
```

The current run passed, proving remaining transcript callers opt into `clanker_message::transcript::...` instead of stable root/default imports.

## Generated inventory and policy

```text
scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 610 public items (615 rows)
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0
```

`policy/embedded-lego/brick-inventory-stability.json` now pins:

```text
total=615
supported=303
optional-support=67
compatibility-alias=0
experimental=184
unsupported-internal=61
stable-contract=370
stable_contract_blake3=bbf41d01f78f5a782ddd0dcda237b7b77a62975c462db7880364b35aa2046e04
```

The stable-contract hash stayed unchanged while unsupported/internal transcript rows left the default scanned surface.
