# session-metrics-capture Specification

## Requirements

### Requirement: Observability kit emits bounded redacted receipts

The observability-audit-receipt-kit SHALL expose only bounded counts and booleans for pending tool state.

#### Scenario: bounded counts and booleans
- GIVEN pending calls exceed receipt limits
- WHEN an audit receipt is generated
- THEN counts MUST be bounded and over-limit state MUST be represented as booleans.

#### Scenario: redacted receipt boundary
- GIVEN tool state may contain raw tool names, call ids, prompts, provider payloads, credentials, authorization headers, OAuth tokens, raw tool arguments, tool output, or secret environment values
- WHEN the receipt is serialized
- THEN those values MUST NOT appear in the receipt.
