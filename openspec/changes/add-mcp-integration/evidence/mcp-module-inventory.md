Artifact-Type: implementation-evidence
Task-ID: phase1-inventory
Covers: r[mcp-integration.capability], r[mcp-integration.observability]
Updated: 2026-04-30T22:22:46Z

# MCP Integration Module Inventory

## Existing ownership points

- `crates/clankers-config/src/settings.rs`
  - Owns merged global + project user settings.
  - Best home for durable MCP server configuration because it already carries `disabled_tools`, hooks, memory, routing, and other user policy.

- `crates/clankers-config/src/paths.rs`
  - Owns global and project config paths.
  - Existing plugin paths are `~/.clankers/agent/plugins/`, `.clankers/plugins/`, and project-root `plugins/`.
  - MCP config should use settings first rather than invent a hidden path; any generated adapter state should live under existing config/data roots.

- `src/modes/common.rs`
  - Owns built-in tool construction, plugin tool construction, tiering, duplicate-name filtering, and `build_all_tiered_tools()`.
  - Best integration point for publishing MCP tools into the agent tool list after config parsing and connection/discovery.

- `src/tools/plugin_tool.rs`
  - Existing generic wrapper around external tool providers.
  - WASM plugin tools call manifest handlers; stdio plugin tools call `start_stdio_tool_call()` and stream progress/results.
  - MCP tools should follow this wrapper pattern, but should get an MCP-specific backend or adapter so MCP protocol concerns do not leak into plugin runtime code.

- `crates/clankers-plugin/src/manifest.rs`
  - Defines Extism/Zellij/Stdio plugin manifests, sandbox fields, permissions, tool definitions, and validation.
  - Reusing plugin manifests for MCP would conflate clankers plugin launch policy with MCP server configuration. Prefer a separate MCP config type and optionally reuse lower-level subprocess/sandbox helpers.

- `crates/clankers-plugin/src/stdio_protocol.rs` and `stdio_runtime.rs`
  - Provide length-prefixed JSON plugin protocol, live tool registration, cancellation, restart/backoff, and active tool discovery.
  - Useful architectural reference for MCP stdio lifecycle, but MCP uses JSON-RPC messages and initialize/listTools/callTool semantics, so a direct protocol reuse is not correct.

- `src/modes/agent_setup.rs`, `src/modes/inline.rs`, `src/modes/json.rs`, `src/modes/print.rs`, `src/modes/daemon/*`
  - These paths receive the tool list built by `build_all_tiered_tools()`.
  - Wiring MCP tools into common tool construction should make them available in prompt, TUI, and daemon sessions with minimal mode-specific code.

## Recommended first-pass architecture

1. Add an MCP configuration model in `clankers-config`:
   - `mcp.servers.<name>.transport = "stdio" | "http"`.
   - stdio: `command`, `args`, `envAllowlist`, optional working directory/sandbox/network policy.
   - http: `url`, optional headers from env allowlist.
   - filters: `includeTools`, `excludeTools`, and optional tool name prefixing.
   - enabled flag and timeout fields.

2. Add an MCP client/runtime module rather than overloading plugin runtime:
   - stdio JSON-RPC transport for initialize, tools/list, tools/call, and cancellation-friendly process shutdown.
   - HTTP transport in a second slice if stdio lands first.
   - normalized `RegisteredTool`-like representation to feed `ToolDefinition`.

3. Add an MCP tool wrapper:
   - `McpTool` stores server name, original MCP tool name, published clankers tool definition, and a runtime handle.
   - Execute calls MCP `tools/call` and maps text/content/error results into `ToolResult`.
   - Emit progress with server/tool identity and redact secrets from error paths.

4. Wire into `src/modes/common.rs`:
   - Build MCP tools after built-ins and before/alongside plugin specialty tools.
   - Reuse duplicate-name filtering so MCP tools cannot shadow built-ins or earlier plugin tools.
   - Respect `disabled_tools` through the existing `ToolSet` path.

5. Verification path:
   - Unit-test config parsing/filtering in `clankers-config`.
   - Unit-test tool name filtering and duplicate handling with a fake MCP registry.
   - Integration-test stdio with a small fake MCP JSON-RPC server process.

## Unsupported first-pass cases to keep explicit

- MCP sampling callbacks to the host model can be deferred unless a concrete server requires them.
- HTTP/SSE streaming can be deferred if stdio JSON-RPC is the first backend.
- Arbitrary environment forwarding should not be supported; use allowlisted names only.
- Tool names that collide with built-ins or already-registered plugin tools should be skipped with a warning, not shadowed.
