## Why

Clankers already has MCP specification and session-control foundations, but Hermes parity requires a durable runtime that can keep MCP servers healthy, refresh tool catalogs safely, and expose actionable receipts when external tool calls fail.

## What Changes

- Add lifecycle and health semantics for stdio and HTTP MCP server runtimes.
- Add safe catalog refresh, schema drift handling, timeout/cancellation, and restart behavior.
- Add receipt metadata that proves calls went through the MCP adapter without leaking arguments or secrets.

## Out of Scope

- Privileged session mutation outside ordinary daemon/session commands.
- Full MCP resource/prompt subscriptions unless explicitly added by a later change.

## Capabilities

### New Capabilities
- `integrations-mcp` follow-up behavior for polish mcp tool runtime.

### Modified Capabilities
- `integrations-mcp` gains implementation-ready requirements for the next Hermes parity slice.

## Impact

- **Files**: OpenSpec artifacts first; implementation tasks identify expected Rust/docs touch points.
- **APIs**: May add CLI flags, tool schemas, settings fields, or daemon/session messages as described in the design.
- **Testing**: Targeted unit/integration checks plus `cargo check --tests` for touched crates.
