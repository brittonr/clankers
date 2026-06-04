# Experimental port budget evidence

Evidence-ID: resolve-experimental-sdk-ports.experimental-port-budget
Artifact-Type: command-output-summary
Task-ID: I1,I2,I4
Covers: embedded-composition-kits.experimental-port-budget, embedded-composition-kits.experimental-port-budget.actionable, embedded-composition-kits.experimental-port-budget.hide-unused
Date: 2026-06-04
Status: PASS

## Implementation summary

- Added `policy/embedded-lego/experimental-sdk-port-budget.json` with owner, use-site status, disposition, validation, rationale, expected stability, and expected row counts for each experimental group.
- Added `scripts/check-experimental-sdk-port-budget.rs` and wired it into `scripts/check-embedded-agent-sdk.rs`, `scripts/emit-embedded-sdk-release-receipt.rs`, and `policy/embedded-lego/brick-inventory-stability.json` release artifacts.
- Removed unused public `clankers-engine-host` observation ports/records that had no production adapter, fixture, or documented product recipe:
  - `EnginePromptBundle`, `EngineHistoryRecord`, `EnginePersistenceRecord`, `EngineHookObservation`, `EngineCostObservation`
  - `PromptHistoryPort`, `PersistencePort`, `HookPort`, `CostAccountingPort`
- Refreshed generated API inventory and brick stability policy after the public surface changed.

## Relevant output

```text
scripts/check-embedded-sdk-api.rs
ok: embedded SDK API inventory covers 586 public items (591 rows)
exit=0

scripts/check-experimental-sdk-port-budget.rs
ok: experimental SDK port budget covers 23 experimental rows; 137 promoted rows
exit=0

scripts/check-brick-inventory-stability.rs
brick-inventory-stability receipt written to target/embedded-sdk-release/brick-inventory-stability-receipt.json
exit=0
```

## Inventory summary

`policy/embedded-lego/brick-inventory-stability.json` now pins:

```text
total=591
supported=440
optional-support=67
compatibility-alias=0
experimental=23
unsupported-internal=61
stable-contract=507
stable_contract_blake3=b53f9a9d34e037c0d883a3b143fe64856f6ca9230f36234cc10264cbf88024ef
```

The remaining experimental rows are budgeted as:

- `clankers-engine::EngineBufferedToolResult` and fields: retained experimental with rationale because reducer buffering is still public through `EngineState` but not yet a direct product recipe.
- `clanker-message` transcript `UserMessage`/`AssistantMessage`/`ToolResultMessage` records and fields: retained experimental under `transcript-compat` compatibility evidence.

The removed engine-host observation port group is tracked with disposition `make-private` and expected public rows `0`, so it cannot silently re-enter inventory.
