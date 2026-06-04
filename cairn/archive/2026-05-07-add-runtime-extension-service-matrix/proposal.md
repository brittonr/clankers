## Why

Auth, provider/router, and plugin runtime services now fail closed by default and run through injected desktop services when present. Each bridge has tests, but embedder safety depends on their combined behavior: absent services must not wake other services, safe receipts must stay redacted, and mixed injected/absent combinations must be deterministic.

## What Changes

- Add a runtime extension service matrix covering auth, provider/router, plugin, MCP/gateway placeholders where applicable, and safe receipt redaction.
- Verify default-safe runtimes fail closed independently for each service.
- Verify mixed injected/absent combinations do not cause hidden discovery, autostart, OAuth, refresh, or raw-output leakage.

## Capabilities

### Modified Capabilities
- `tool-host-embedding`: runtime extension service publication/execution receives combined matrix acceptance.

## Impact

- **Files**: runtime services tests/fixtures, redaction assertions, embedded SDK acceptance script.
- **APIs**: no public API changes unless test-only fake service hooks are needed.
- **Testing**: focused runtime services tests plus embedded SDK acceptance.
