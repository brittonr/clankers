## ADDED Requirements

### Requirement: MCP Runtime Lifecycle [r[mcp-runtime.lifecycle]]
The system MUST manage configured MCP server processes or HTTP clients through a bounded lifecycle with health, restart, timeout, and cancellation semantics.

#### Scenario: Healthy server publishes tools [r[mcp-runtime.lifecycle.scenario.healthy-server-publishes-tools]]
- GIVEN an enabled MCP server validates and initializes
- WHEN the shared tool registry is built
- THEN the server's filtered tools are published with a healthy runtime state

#### Scenario: Failed server is isolated [r[mcp-runtime.lifecycle.scenario.failed-server-is-isolated]]
- GIVEN one configured MCP server fails initialization
- WHEN other configured servers and built-in tools are available
- THEN clankers skips only the failed server and returns an actionable warning for that server

### Requirement: MCP Catalog Refresh and Drift Handling [r[mcp-runtime.catalog-refresh]]
The system MUST refresh MCP tool catalogs deterministically and fail closed when a tool schema changes during a session.

#### Scenario: Refresh preserves stable tool names [r[mcp-runtime.catalog-refresh.scenario.refresh-preserves-stable-tool-names]]
- GIVEN a server returns the same tool names with equivalent schemas
- WHEN a refresh runs
- THEN clankers keeps the visible tool names stable

#### Scenario: Schema drift is explicit [r[mcp-runtime.catalog-refresh.scenario.schema-drift-is-explicit]]
- GIVEN a server changes a published tool schema incompatibly
- WHEN a call arrives using the old schema
- THEN clankers rejects the call with a schema-drift error instead of sending malformed input

### Requirement: MCP Runtime Receipts [r[mcp-runtime.receipts]]
The system MUST attach safe MCP runtime receipts to tool results and logs.

#### Scenario: Receipt excludes secrets [r[mcp-runtime.receipts.scenario.receipt-excludes-secrets]]
- GIVEN an MCP call includes arguments or headers that may contain secrets
- WHEN the call completes or fails
- THEN the receipt includes server, visible tool, original MCP tool, status, duration, and redacted error class without raw arguments, headers, tokens, or environment values
