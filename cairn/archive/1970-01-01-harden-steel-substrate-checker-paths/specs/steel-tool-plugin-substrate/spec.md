# Steel Tool Plugin Substrate Delta

## Purpose

Keeps the Steel tool/plugin/subagent substrate checker usable as a durable archived regression rail.

## Requirements

### Requirement: Substrate checker resolves archived lifecycle artifacts [r[steel-tool-plugin-substrate.checker-paths]]

The Steel tool/plugin/subagent substrate checker MUST remain runnable after the original `steel-tool-plugin-substrate` Cairn change has been archived.

#### Scenario: active-to-archive path resolution [r[steel-tool-plugin-substrate.checker-paths.active-archive-resolution]]
- GIVEN `cairn/changes/steel-tool-plugin-substrate/` is absent after archive
- WHEN `scripts/check-steel-tool-plugin-substrate.rs` runs
- THEN it MUST validate the archived task ledger and canonical `steel-tool-plugin-substrate` specification instead of failing on missing active-change paths
- AND it MUST still prefer active task/spec paths when those paths exist for future in-progress changes

#### Scenario: receipt hashes resolved lifecycle artifacts [r[steel-tool-plugin-substrate.checker-paths.receipt-artifacts]]
- GIVEN the checker resolves archived or active lifecycle paths
- WHEN it writes `target/steel-tool-plugin-substrate/receipt.json`
- THEN the receipt MUST hash the resolved task and specification artifacts that were actually validated
- AND it MUST preserve the existing redacted checker receipt schema without raw prompts, provider payloads, tool bodies, credentials, or UCAN proofs
