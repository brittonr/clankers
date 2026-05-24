# Steel Eval Live Tool Discovery Receipt Specification

## Purpose

Defines the `steel-eval-live-tool-discovery-receipt` capability.

## Requirements

### Requirement: Default-present runtime receipt [r[steel-eval-live-tool-discovery-receipt.default-present-receipt]]

Clankers MUST be able to produce product-level evidence that `steel_eval` appears in an actual runtime tool catalog under default settings.

#### Scenario: Runtime catalog includes Steel eval
- GIVEN Clankers starts with default settings
- WHEN a product-level tool catalog or equivalent runtime discovery path is queried
- THEN the receipt MUST assert `steel_eval` is present

### Requirement: Hidden-path runtime receipt [r[steel-eval-live-tool-discovery-receipt.hidden-receipt]]

Clankers MUST be able to produce product-level evidence that opt-out or disabled-tool policy hides `steel_eval`.

#### Scenario: Runtime catalog hides Steel eval
- GIVEN settings or disabled-tool policy hide `steel_eval`
- WHEN the same runtime discovery path is queried
- THEN the receipt MUST assert `steel_eval` is absent

### Requirement: Safe discovery receipt [r[steel-eval-live-tool-discovery-receipt.safe-receipt]]

Steel eval discovery receipts MUST avoid prompts, secrets, host output, and mutation evidence.

#### Scenario: Discovery receipt is metadata-only
- GIVEN the runtime discovery proof is recorded
- WHEN the receipt is inspected
- THEN it MUST contain safe metadata and assertions only
- AND MUST NOT execute Steel source or expose credentials
