# Verification Notes

## Integration Inventory

- Tool Gateway policy core: `src/tool_gateway.rs` owns target parsing, toolset validation, active toolset filtering, delivery request/attempt/receipt models, local/session adapter, Matrix active-context adapter, local outbox persistence, status lookup, and retry-by-attempt-id.
- CLI surface: `src/cli.rs` and `src/commands/gateway.rs` expose `gateway deliver`, `gateway delivery-status`, `gateway retry`, and the compatibility `gateway deliver-receipt` path.
- Agent tool surface: `src/tools/tool_gateway.rs` exposes `status`, `validate`, `deliver`, `deliver_receipt`, `delivery_status`, and `retry` actions with replay-safe `details` metadata.
- Matrix boundary: this slice intentionally uses an explicit active-context adapter seam and redacted handles. Raw Matrix destinations, webhooks, cloud targets, and Matrix without active context fail closed before platform dispatch.
- Artifact handoff: file/media/scheduled-output labels flow through `DeliveryRequest`/`DeliveryAttempt`; outboxes store basename-only artifact labels and safe attempt ids, not raw paths or payload bytes.

## Focused Verification

- `cargo fmt --check`
- `CARGO_TARGET_DIR=target cargo test --lib gateway -- --nocapture`
- `CARGO_TARGET_DIR=target cargo test --test gateway -- --nocapture`
- `CARGO_TARGET_DIR=target cargo check --tests`
- CLI smoke: `gateway deliver` recorded a scheduled-output outbox attempt, `gateway delivery-status` read it, and `gateway deliver --deliver matrix --matrix-active` produced a JSON Matrix adapter receipt. The outbox was checked not to contain `secret` or the raw Matrix binding domain.

## Redaction Findings

Receipts and outboxes include source/action/status/backend, artifact type, target kind, attempt id, basename-only `safe_path`, optional redacted platform handle, retryability, and sanitized error metadata. They do not persist raw destinations, full paths, tokens, headers, Matrix room identifiers, message payloads, or artifact bytes.
