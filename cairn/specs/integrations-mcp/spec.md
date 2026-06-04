# MCP Integrations Specification

## Purpose

This specification defines how clankers connects to Model Context Protocol (MCP) servers, publishes MCP tools through the normal clankers tool surface, manages runtime health, and records safe metadata for audit and troubleshooting without bypassing daemon/session/tool policy.

## Requirements

### Requirement: MCP Integration Capability

The system MUST connect clankers to stdio and HTTP MCP servers with per-server tool filtering and safe tool publication.

#### Scenario: Primary path succeeds

- GIVEN clankers is configured for the capability
- WHEN the user or agent invokes the documented primary path
- THEN clankers performs the operation and returns a structured, user-visible result

#### Scenario: Unsupported configuration is explicit

- GIVEN the user invokes the capability without required configuration or platform support
- WHEN clankers cannot safely proceed
- THEN clankers MUST return an actionable error instead of silently falling back or dropping work

### Requirement: MCP Server Configuration

The system MUST define MCP servers in merged clankers settings under `mcp.servers` with explicit transport, filtering, timeout, and environment-forwarding policy.

#### Scenario: Stdio server config is accepted

- GIVEN settings define an enabled stdio MCP server with `command`, optional `args`, tool filters, and an environment allowlist
- WHEN clankers loads settings
- THEN the MCP server configuration is represented without requiring plugin manifests

#### Scenario: HTTP server config is accepted

- GIVEN settings define an enabled HTTP MCP server with `url`, optional redacted header environment mappings, and tool filters
- WHEN clankers loads settings
- THEN the MCP server configuration is represented separately from stdio launch policy

#### Scenario: Unsafe environment forwarding is rejected

- GIVEN an MCP server needs secrets or headers
- WHEN the user configures the server
- THEN clankers MUST only forward explicitly allowlisted environment variables or header environment mappings

### Requirement: MCP Tool Publication

The system MUST publish MCP tools with deterministic clankers-visible names that identify their source server and avoid shadowing existing tools.

#### Scenario: Tool names are prefixed

- GIVEN an MCP server named `filesystem` exposes `read_file`
- WHEN clankers publishes the MCP tool without a custom prefix
- THEN the visible tool name is `mcp_filesystem_read_file`

#### Scenario: Tool filters apply before publication

- GIVEN `includeTools` and `excludeTools` are configured
- WHEN the MCP server returns a tool list
- THEN clankers MUST publish only tools allowed by the filter policy

#### Scenario: Tool collisions do not shadow existing tools

- GIVEN an MCP tool would publish with the same name as a built-in or already registered plugin tool
- WHEN clankers builds the tool list
- THEN clankers MUST skip the MCP tool and record a warning rather than shadowing the existing tool

### Requirement: MCP Runtime Lifecycle

The system MUST manage configured MCP server processes or HTTP clients through bounded lifecycle semantics with health, timeout, and cancellation behavior.

#### Scenario: Healthy server publishes tools

- GIVEN an enabled MCP server validates and initializes
- WHEN the shared tool registry is built
- THEN the server's filtered tools are published with a healthy runtime state

#### Scenario: Failed server is isolated

- GIVEN one configured MCP server fails initialization
- WHEN other configured servers and built-in tools are available
- THEN clankers MUST skip only the failed server and return an actionable warning for that server

### Requirement: MCP Catalog Refresh and Drift Handling

The system MUST refresh MCP tool catalogs deterministically and fail closed when a tool schema changes during a session.

#### Scenario: Refresh preserves stable tool names

- GIVEN a server returns the same tool names with equivalent schemas
- WHEN a refresh runs
- THEN clankers keeps the visible tool names stable

#### Scenario: Schema drift is explicit

- GIVEN a server changes a published tool schema incompatibly
- WHEN a call arrives using the old schema
- THEN clankers MUST reject the call with a schema-drift error instead of sending malformed input

### Requirement: MCP Runtime Receipts

The system MUST attach safe MCP runtime receipts to tool results and logs.

#### Scenario: Receipt excludes secrets

- GIVEN an MCP call includes arguments or headers that may contain secrets
- WHEN the call completes or fails
- THEN the receipt includes server, visible tool, original MCP tool, status, duration, and redacted error class without raw arguments, headers, tokens, or environment values

### Requirement: MCP Integration Session Observability

The system MUST record enough normalized metadata for audit, replay, and troubleshooting without leaking secrets.

#### Scenario: Session records useful metadata

- GIVEN the capability runs inside a persisted session
- WHEN the operation completes or fails
- THEN the session record includes status, timing or backend identity when useful, and redacted error details when applicable

### Requirement: MCP Integration Verification

The implementation MUST include automated tests and documentation for the supported first-pass behavior.

#### Scenario: Regression suite covers happy and failure paths

- GIVEN the feature is implemented
- WHEN the targeted test suite runs
- THEN tests cover at least one successful operation and one policy/configuration failure
