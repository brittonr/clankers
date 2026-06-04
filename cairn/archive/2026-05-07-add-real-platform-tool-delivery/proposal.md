## Why

The current Tool Gateway validates toolsets and emits platform-safe delivery receipts, but it intentionally does not deliver artifacts to real platform destinations. That keeps the first pass safe, but it leaves generated files, media, and scheduled outputs as local/session-only evidence rather than actual user-visible delivery.

## What Changes

- **Delivery adapter boundary**: Add a shared delivery adapter interface behind the Tool Gateway for local/session delivery plus explicitly configured platform adapters.
- **Matrix first platform**: Implement Matrix delivery only when the active session has an explicit Matrix bridge context and policy accepts the target.
- **Outbox and retries**: Persist bounded delivery attempts in a local outbox so failures can be retried without resending raw secrets.
- **Receipt parity**: Keep all delivery receipts replay-safe and free of destination secrets, headers, tokens, raw payloads, and full paths.

## Capabilities

### Modified Capabilities
- `tool-gateway-platform-delivery`: Tool Gateway evolves from validation/receipt-only delivery to guarded adapter-backed delivery for approved platform contexts.

## Impact

- **Files**: likely `src/tool_gateway.rs`, `src/tools/tool_gateway.rs`, `src/commands/gateway.rs`, Matrix/session delivery seams, scheduled output hooks, tests, and docs.
- **APIs**: may add delivery adapter traits, outbox receipt models, CLI subcommands/flags for retry/status, and tool action fields for delivery attempts.
- **Dependencies**: prefer existing Matrix/session crates and filesystem persistence; no new remote SDK until an adapter proves the need.
- **Testing**: unit tests for policy/outbox/receipt redaction, fake adapter integration tests, Matrix-context negative tests, and one deterministic end-to-end smoke that writes an artifact and records a delivered/failed-safe receipt.
