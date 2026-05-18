## Why

Durable process/job support needs a stable agent-facing contract before implementation starts. The existing `process` tool already has useful semantics, but adding durable backends, profiles, retention, and notifications without pinning request/receipt shapes would couple callers to backend-specific text and make TUI/daemon/remote integrations drift.

## What Changes

- **Typed process/job API**: Define request parameters and actions for existing `process` behavior plus durable job options.
- **Shared receipt schema**: Require machine-readable receipts/errors for start, list, poll, log, wait, kill, restart, stdin, adoption, profile start, and GC.
- **BLAKE3-native identity**: Require public `ProcessJobId` values to be derived from canonical, versioned BLAKE3 identity envelopes, with backend locators carried separately.
- **Backend-neutral DTOs**: Route public API requests through shared DTOs before native, pueue, or systemd dispatch.
- **Compatibility guardrails**: Preserve existing native process semantics where callers do not request durable-only options.

## Capabilities

### New Capabilities

- `process-job-tool-api`: Stable agent-facing process/job request and receipt contract.

### Modified Capabilities

- `durable-process-jobs`: Uses this API as the public surface for backend-neutral durable process/job management.

## Impact

- **Files likely affected**: `src/tools/process.rs`, process/job DTO modules, daemon event DTOs, `crates/clanker-tui-types`, tests for tool JSON schema/receipts.
- **APIs**: Existing `process` actions remain; new optional fields and typed receipts are added in a backwards-compatible way where possible.
- **Testing**: Golden request/receipt fixtures, BLAKE3 identity fixtures, unsupported-action errors, native compatibility tests, projection tests.
