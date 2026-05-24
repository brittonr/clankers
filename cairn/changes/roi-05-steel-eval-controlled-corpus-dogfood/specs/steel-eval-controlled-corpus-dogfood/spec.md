## ADDED Requirements

### Requirement: Controlled corpus manifest [r[steel-eval-controlled-corpus-dogfood.corpus-manifest]]

Steel eval dogfood MUST use an explicit local corpus manifest before making controlled or promotion-eligible claims.

#### Scenario: Missing corpus blocks dogfood claim
- GIVEN a Steel eval dogfood run lacks a valid local corpus manifest
- WHEN readiness is computed
- THEN the receipt MUST mark the dogfood claim blocked

### Requirement: Threshold and regression budget [r[steel-eval-controlled-corpus-dogfood.threshold-budget]]

Steel eval dogfood MUST evaluate improvements and regressions against reviewed thresholds.

#### Scenario: Regression budget controls recommendation
- GIVEN corpus cases produce candidate scores
- WHEN regressions exceed the reviewed budget or improvements miss the minimum
- THEN the receipt MUST mark the run not recommended

### Requirement: Dogfood receipt taxonomy [r[steel-eval-controlled-corpus-dogfood.receipt-taxonomy]]

Steel eval dogfood receipts MUST distinguish pass, blocked, unchanged/noise, regression, evaluation failure, and redaction outcomes.

#### Scenario: Receipt records outcome class
- GIVEN a dogfood run completes or fails
- WHEN the receipt is inspected
- THEN it MUST include a deterministic outcome class and safe issue codes without raw sensitive corpus content

### Requirement: No dogfood authority expansion [r[steel-eval-controlled-corpus-dogfood.no-authority-expansion]]

Controlled Steel eval dogfood MUST NOT grant mutation, remote fetch, credentials, or default host authority.

#### Scenario: Dogfood keeps pure authority boundary
- GIVEN controlled dogfood executes Steel eval
- WHEN the profile is selected
- THEN mutation and ambient host authority MUST remain denied unless a separate reviewed profile grants them
