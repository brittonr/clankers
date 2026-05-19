# Session Metrics Capture Specification

## Purpose

Defines reusable session metrics and audit receipt contracts for safe, bounded observability across controller-owned prompt, tool, plugin, and session telemetry.

## Requirements

### Requirement: Observability kit emits bounded redacted receipts [r[session-metrics-capture.observability-audit-receipt-kit]]

The system MUST define `observability-audit-receipt-kit` as a composable Clankers brick with explicit ownership boundaries, deterministic fixtures, and safe evidence.

#### Scenario: Brick boundary is explicit [r[session-metrics-capture.observability-audit-receipt-kit.boundary]]

- GIVEN a product or contributor adopts the `observability-audit-receipt-kit` brick
- WHEN the brick is documented, instantiated, or validated
- THEN the contract MUST name which behavior is reusable, which behavior stays product-owned, and which shell/runtime systems are out of scope
- THEN reusable receipt construction MUST stay separate from product-owned persistence, export, daemon, plugin, provider, and TUI sinks

#### Scenario: Receipts are bounded and redacted [r[session-metrics-capture.observability-audit-receipt-kit.redaction]]

- GIVEN audit or metrics state contains pending tool calls, completed tool calls, or dropped-event counters
- WHEN a reusable observability receipt is produced
- THEN the receipt MUST expose bounded counts and booleans rather than hidden maps, raw event payloads, or unbounded pending buffers
- THEN the receipt MUST NOT serialize raw tool names, call ids, prompts, provider payloads, credentials, authorization headers, OAuth tokens, raw tool arguments, tool output, or secret environment values
- WHEN hidden pending state exceeds the public receipt limit
- THEN the receipt MUST clamp public count fields to the configured limit and expose an over-limit diagnostic boolean

#### Scenario: Brick has executable evidence [r[session-metrics-capture.observability-audit-receipt-kit.evidence]]

- GIVEN the brick is changed
- WHEN the focused verification for the change runs
- THEN it MUST exercise at least one positive path and one fail-closed or negative path through deterministic fixtures, examples, policy checks, generated inventory checks, or receipt validation
- THEN evidence MUST be safe to commit or summarize without raw prompts, credentials, authorization headers, OAuth tokens, provider payloads, hidden context, raw tool arguments, or secret environment values

#### Scenario: Brick drift is diagnosable [r[session-metrics-capture.observability-audit-receipt-kit.drift]]

- GIVEN source code, docs, fixtures, policy, or generated inventories drift apart
- WHEN the brick validation rail runs
- THEN it MUST fail with a diagnostic that names the stale artifact and the expected owner of the update
- THEN intentional contract changes MUST require updating tests, docs, and receipt or fixture evidence together
