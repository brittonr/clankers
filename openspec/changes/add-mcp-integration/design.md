## Context

This change tracks Hermes feature-parity work for MCP Integration. Clankers already has strong Rust-native agent, daemon, plugin, routing, scheduling, and tool foundations; this change should compose with those foundations rather than bypass them.

## Goals / Non-Goals

**Goals:**
- Provide a small, testable first implementation that is useful from the TUI, prompt mode, and daemon/session paths.
- Keep policy decisions explicit: credentials, sandboxing, persistence, and output delivery must be auditable.
- Document gaps intentionally left for follow-up.

**Non-Goals:**
- Large rewrites of the agent loop or provider stack unless required by the capability boundary.
- Hidden best-effort behavior that silently drops outputs, credentials, or session context.

## Decisions

### 1. Build on existing clankers primitives

**Choice:** Reuse existing tool registration, daemon/session persistence, config paths, provider routing, and plugin/runtime abstractions where possible.

**Rationale:** This keeps the feature consistent with clankers architecture and avoids Hermes-shaped islands that are hard to maintain.

**Alternative:** Copy Hermes behavior directly as a separate subsystem. Rejected because duplicated lifecycle and policy handling would drift quickly.

**Implementation:** Add the minimum new module/crate surface needed for MCP Integration, then wire it through the existing CLI/TUI/daemon paths.

### 2. Make policy and observability first-class

**Choice:** Every implementation MUST expose enough state for tests, logs, session replay, and user-facing errors.

**Rationale:** These features often cross process, network, or file boundaries. Silent fallback is harder to debug than a clear unsupported-path error.

**Alternative:** Optimize only for a happy-path demo. Rejected because these are agent autonomy features and failures must be recoverable.

### 3. User-facing MCP surface

**Choice:** Configure MCP servers through merged clankers settings under an `mcp.servers` map. Each server entry has:

```json
{
  "mcp": {
    "servers": {
      "filesystem": {
        "enabled": true,
        "transport": "stdio",
        "command": "npx",
        "args": ["-y", "@modelcontextprotocol/server-filesystem", "/tmp"],
        "envAllowlist": ["MCP_TOKEN"],
        "includeTools": ["read_file", "write_file"],
        "excludeTools": [],
        "toolPrefix": "fs",
        "timeoutMs": 30000
      }
    }
  }
}
```

HTTP servers use `transport: "http"`, `url`, and optional `headerEnv` for redacted header values. Stdio and HTTP are represented by one config enum so unsupported/missing fields fail during settings validation or MCP startup with actionable messages.

**Rationale:** Settings are already global/project merged, daemon-compatible, and familiar to clankers users. A map keyed by server name supports per-server filtering, collision diagnostics, and future CLI/TUI management without committing to a new file format.

**Alternative:** Store MCP servers as plugin manifests. Rejected because MCP servers are external JSON-RPC services, not clankers plugins; treating them as plugins would blur sandbox, protocol, and lifecycle semantics.

### 4. Published tool naming and filtering

**Choice:** By default MCP tools publish as `mcp_<server>_<tool>`. If `toolPrefix` is set, publish as `<prefix>_<tool>`. `includeTools` filters before `excludeTools`; duplicate names and built-in/plugin collisions are skipped with warnings.

**Rationale:** MCP server tool names are not globally unique and may collide with core tools such as `read`, `write`, or `search`. Prefixing keeps the LLM-visible surface explicit and safe.

**Alternative:** Publish original MCP names directly. Rejected for collision risk and unclear provenance in transcripts.

### 5. First-pass unsupported cases

**Choice:** The first implementation MUST return explicit unsupported errors for MCP sampling callbacks, resource subscriptions, prompt templates, server-initiated roots/list changes, and streaming HTTP/SSE if not implemented in the first backend slice.

**Rationale:** These protocol features are valuable, but tool invocation can land first without pretending the full MCP spec is supported.

**Alternative:** Ignore unsupported server requests. Rejected because silent protocol gaps are hard to debug and can make external tools appear unreliable.

## Risks / Trade-offs

**Scope creep** → Start with a minimal backend/API and document additional backends as future tasks.

**Security regressions** → Reuse sanitized environments, capability checks, and explicit allowlists.

**Session replay drift** → Store normalized events/metadata rather than backend-specific blobs when possible.
