## ADDED Requirements

### Requirement: Host-owned extension services for embedding [r[embeddable-agent-engine.extension-services.host-owned]]

The system MUST let embedding hosts provide explicit extension services for provider routing, auth-store access, credential-pool policy, and plugin/MCP runtime lifecycle instead of requiring the runtime boundary to discover or mutate Clankers desktop defaults directly.

#### Scenario: embedded host supplies provider and auth services [r[embeddable-agent-engine.extension-services.host-owned.provider-auth]]

- GIVEN an embedding host constructs a Clankers runtime with host-owned provider routing and auth services
- WHEN a model request needs provider execution
- THEN the runtime delegates through the host-supplied provider/router service rather than autostarting the desktop router daemon or reading desktop auth paths directly
- THEN auth lookup, credential-pool selection, token refresh persistence, and account scoping follow the host-supplied service policy

#### Scenario: embedded host supplies plugin lifecycle services [r[embeddable-agent-engine.extension-services.host-owned.plugin-runtime]]

- GIVEN an embedding host enables plugin or MCP tools
- WHEN a plugin/MCP-backed tool is published or executed
- THEN plugin process startup, MCP server lifecycle, environment/header allowlists, sandbox policy, and shutdown are owned by the host-supplied extension service
- THEN the core runtime boundary does not spawn plugin subprocesses, start MCP servers, or inherit host environment variables implicitly

### Requirement: Extension services default to no side effects in embedded mode [r[embeddable-agent-engine.extension-services.default-safe]]

The embedded runtime MUST default side-effectful extension systems to disabled or fail-closed behavior unless the host explicitly enables and supplies the needed service.

#### Scenario: disabled extensions do not start external systems [r[embeddable-agent-engine.extension-services.default-safe.no-startup]]

- GIVEN an embedding host uses default-safe runtime construction
- WHEN the host creates a session, assembles a prompt, builds a tool catalog, or sends a model request
- THEN Clankers does not autostart the router daemon, open OAuth login flows, write pending login verifiers, persist refreshed credentials, launch plugin subprocesses, connect to MCP servers, or start gateway delivery paths
- THEN requests requiring an absent extension service fail with an actionable unsupported or unavailable error

#### Scenario: explicit desktop adapter preserves current defaults [r[embeddable-agent-engine.extension-services.default-safe.desktop-opt-in]]

- GIVEN normal Clankers CLI, TUI, daemon, ACP, or MCP shells want current desktop behavior
- WHEN those shells construct runtime services through the desktop adapter
- THEN router discovery/autostart, provider-scoped auth, credential pools, plugin discovery, MCP publication, and gateway behavior remain available according to existing settings and policy
- THEN that availability is an explicit adapter choice rather than an implicit behavior of the reusable runtime boundary

### Requirement: Extension service metadata is safe and replayable [r[embeddable-agent-engine.extension-services.safe-metadata]]

Extension services MUST emit safe replay/debug metadata and receipts that identify extension source, action, status, timing, and error class without leaking credentials or provider/plugin payloads.

#### Scenario: provider/router/auth metadata redacts sensitive data [r[embeddable-agent-engine.extension-services.safe-metadata.provider-auth]]

- GIVEN a provider/router/auth operation succeeds or fails
- WHEN the runtime records replay/debug metadata for that operation
- THEN metadata may include provider name, account label, model label, route source, status, duration, and safe error class
- THEN metadata MUST NOT include API keys, OAuth tokens, refresh tokens, authorization headers, raw provider request or response bodies, login verifier secrets, credential file contents, or environment values

#### Scenario: plugin/MCP metadata redacts sensitive data [r[embeddable-agent-engine.extension-services.safe-metadata.plugin-mcp]]

- GIVEN a plugin or MCP-backed tool is published or executed
- WHEN the runtime records replay/debug metadata for that operation
- THEN metadata may include plugin/server name, visible tool name, original tool name, runtime kind, status, duration, and safe error class
- THEN metadata MUST NOT include raw tool arguments, raw tool output, headers, tokens, environment values, subprocess command secrets, or plugin state file contents

### Requirement: Desktop adapter parity rails cover extension services [r[embeddable-agent-engine.extension-services.desktop-adapter-parity]]

The system MUST verify that desktop Clankers shells preserve existing provider/router/auth/plugin behavior through explicit extension-service adapters.

#### Scenario: provider routing parity is tested through the adapter [r[embeddable-agent-engine.extension-services.desktop-adapter-parity.provider-router]]

- GIVEN current desktop Clankers provider settings and auth-store inputs
- WHEN the desktop adapter constructs provider/router services
- THEN tests prove existing provider discovery, explicit known-provider fail-closed behavior, OpenAI Codex separation, OpenAI-compatible routing, and credential-pool selection remain equivalent to the current paths

#### Scenario: plugin publication parity is tested through the adapter [r[embeddable-agent-engine.extension-services.desktop-adapter-parity.plugin-publication]]

- GIVEN current desktop Clankers plugin/MCP/gateway settings
- WHEN the desktop adapter constructs extension runtime services and the tool catalog asks for extension-backed tools
- THEN tests prove enabled plugin/MCP/gateway tools publish through the existing default policy
- THEN disabled, invalid, unhealthy, or unavailable extension runtimes fail closed without publishing broken tools
