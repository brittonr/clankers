## ADDED Requirements

### Requirement: MCP Integration Capability [r[mcp-integration.capability]]
The system MUST connect clankers to stdio and HTTP MCP servers with per-server tool filtering and safe tool publication.

#### Scenario: Primary path succeeds [r[mcp-integration.scenario.primary-path]]
- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit [r[mcp-integration.scenario.unsupported-config]]
- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: MCP Server Configuration [r[mcp-integration.configuration]]
The system MUST define MCP servers in merged clankers settings under `mcp.servers` with explicit transport, filtering, timeout, and environment-forwarding policy.

#### Scenario: Stdio server config is accepted [r[mcp-integration.scenario.stdio-config]]
- GIVEN settings define an enabled stdio MCP server with `command`, optional `args`, tool filters, and an environment allowlist
- WHEN clankers loads settings
- THEN the MCP server configuration is represented without requiring plugin manifests

#### Scenario: HTTP server config is accepted [r[mcp-integration.scenario.http-config]]
- GIVEN settings define an enabled HTTP MCP server with `url`, optional redacted header environment mappings, and tool filters
- WHEN clankers loads settings
- THEN the MCP server configuration is represented separately from stdio launch policy

#### Scenario: Unsafe environment forwarding is rejected [r[mcp-integration.scenario.env-policy]]
- GIVEN an MCP server needs secrets or headers
- WHEN the user configures the server
- THEN clankers MUST only forward explicitly allowlisted environment variables or header environment mappings

### Requirement: MCP Tool Publication [r[mcp-integration.tool-publication]]
The system MUST publish MCP tools with deterministic clankers-visible names that identify their source server and avoid shadowing existing tools.

#### Scenario: Tool names are prefixed [r[mcp-integration.scenario.prefixed-tools]]
- GIVEN an MCP server named `filesystem` exposes `read_file`
- WHEN clankers publishes the MCP tool without a custom prefix
- THEN the visible tool name is `mcp_filesystem_read_file`

#### Scenario: Tool filters apply before publication [r[mcp-integration.scenario.tool-filters]]
- GIVEN `includeTools` and `excludeTools` are configured
- WHEN the MCP server returns a tool list
- THEN clankers MUST publish only tools allowed by the filter policy

#### Scenario: Tool collisions do not shadow existing tools [r[mcp-integration.scenario.tool-collisions]]
- GIVEN an MCP tool would publish with the same name as a built-in or already registered plugin tool
- WHEN clankers builds the tool list
- THEN clankers MUST skip the MCP tool and record a warning rather than shadowing the existing tool

### Requirement: MCP Integration Session Observability [r[mcp-integration.observability]]
The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata [r[mcp-integration.scenario.session-metadata]]
- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: MCP Integration Verification [r[mcp-integration.verification]]
The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths [r[mcp-integration.scenario.regression-suite]]
- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure

