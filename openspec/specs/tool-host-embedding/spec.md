# tool-host-embedding Specification

## Purpose
TBD - created by archiving change extract-tool-catalog-capability-packs. Update Purpose after archive.
## Requirements
### Requirement: Reusable tool catalog builder [r[tool-host-embedding.catalog-builder]]

The system MUST provide a reusable tool catalog builder that constructs Clankers tool registries outside CLI, TUI, daemon, ACP, or MCP mode modules and allows hosts to select built-in tool packs, disabled tools, optional extension runtimes, and custom tools explicitly. Catalog construction MUST NOT start provider routers, OAuth flows, plugin subprocesses, MCP servers, or gateway delivery paths as an implicit side effect.

#### Scenario: host builds a read-only catalog [r[tool-host-embedding.catalog-builder.readonly]]

- GIVEN an embedding host requests a read-only capability profile
- WHEN the host builds the tool catalog
- THEN the catalog includes only tools that cannot mutate files, spawn processes, send network requests, alter git state, start plugins, or deliver artifacts
- THEN mutating tools such as `write`, `edit`, `patch`, `bash`, `process`, `commit`, browser actions, plugin tools, MCP tools, and gateway delivery are absent unless separately enabled

#### Scenario: Clankers default catalog remains reproducible [r[tool-host-embedding.catalog-builder.default-parity]]

- GIVEN the normal Clankers CLI/TUI/daemon default policy
- WHEN the builder constructs the default Clankers catalog through explicit desktop extension adapters
- THEN the published built-in tools, tiers, optional tools, plugin tools, MCP tools, disabled-tool filtering, and gateway policy match the existing user-visible default surface

#### Scenario: catalog metadata query does not start extension runtimes [r[tool-host-embedding.catalog-builder.no-eager-extension-startup]]

- GIVEN an embedding host asks for catalog metadata with plugin, MCP, router, or gateway packs disabled or absent
- WHEN the catalog builder constructs descriptors for host inspection
- THEN it does not start plugin subprocesses, connect to MCP servers, autostart router daemons, open OAuth flows, or start gateway delivery paths
- THEN unavailable extension-backed tools are omitted or reported as unavailable in safe metadata according to host policy

### Requirement: Capability packs [r[tool-host-embedding.capability-packs]]

The tool catalog MUST expose named capability packs for coarse host policy selection, and each pack MUST document its side-effect class, prerequisites, and default embedding status.

#### Scenario: dangerous packs are opt-in [r[tool-host-embedding.capability-packs.dangerous-opt-in]]

- GIVEN an embedding host uses default-safe runtime construction
- WHEN the tool catalog is built
- THEN shell/process execution, filesystem mutation, git mutation, browser automation, plugin process startup, MCP external servers, Matrix actions, provider-router daemon autostart, auth/OAuth flows, credential refresh persistence, and gateway delivery are not published or activated by default
- THEN enabling one dangerous pack does not implicitly enable unrelated dangerous packs

#### Scenario: pack filtering composes with disabled tools [r[tool-host-embedding.capability-packs.disabled-filter]]

- GIVEN a host enables a capability pack and also disables a named tool
- WHEN the tool catalog is built
- THEN the disabled tool is omitted even if its pack is enabled
- THEN the omission is visible in safe catalog metadata for debugging

### Requirement: Host custom tools [r[tool-host-embedding.custom-tools]]

The catalog builder MUST allow embedding hosts to register app-native tools with explicit schemas, source labels, tiering, collision policy, and execution handles.

#### Scenario: host tool coexists with Clankers tools [r[tool-host-embedding.custom-tools.coexist]]

- GIVEN a host registers an app-native tool and enables selected Clankers packs
- WHEN the catalog is built
- THEN the host tool appears with its source label and schema alongside enabled Clankers tools
- THEN name collisions are rejected or resolved according to explicit host policy rather than silently overriding a tool

### Requirement: Extension runtime publication is explicit [r[tool-host-embedding.extension-runtime.explicit-publication]]

The catalog builder MUST publish plugin, MCP, router-backed, auth-backed, and gateway-backed tools only when the host explicitly enables the corresponding capability pack and supplies or selects an extension runtime service that is allowed to publish that tool.

#### Scenario: absent extension runtime omits extension-backed tools [r[tool-host-embedding.extension-runtime.absent-runtime]]

- GIVEN a host enables ordinary built-in read-only tools but does not supply plugin, MCP, router, auth, or gateway extension services
- WHEN the tool catalog is built
- THEN plugin tools, MCP tools, router-backed provider tools, auth-management tools that mutate stores, and gateway delivery tools are absent or marked unavailable according to host policy
- THEN no subprocess, server connection, credential-store write, or delivery attempt occurs while building the catalog

#### Scenario: enabled extension runtime carries safe source metadata [r[tool-host-embedding.extension-runtime.safe-source-metadata]]

- GIVEN a host supplies an extension runtime service and enables an extension-backed tool pack
- WHEN the catalog publishes tools from that service
- THEN each descriptor identifies the safe source class, visible tool name, original extension tool identity when applicable, side-effect class, and prerequisites
- THEN descriptors MUST NOT include credentials, headers, environment values, raw plugin arguments, provider request bodies, or plugin state contents

### Requirement: Plugin Runtime Extension Execution [r[tool-host-embedding.plugin-runtime-execution]]

The system MUST support an explicitly injected plugin runtime extension service that can publish plugin tool descriptors and execute a plugin tool without letting the embedded runtime discover plugin roots or launch extension runtimes implicitly.

#### Scenario: Desktop host injects plugin runtime [r[tool-host-embedding.plugin-runtime-execution.desktop-injected]]

- GIVEN a desktop host supplies a plugin manager to the runtime service adapter
- WHEN the host asks the extension runtime service for plugin publishable tools
- THEN descriptors are derived from that injected manager without using hidden runtime discovery

#### Scenario: Plugin tool execution returns safe receipt [r[tool-host-embedding.plugin-runtime-execution.safe-receipt]]

- GIVEN a desktop host supplies a plugin manager containing a loaded plugin tool
- WHEN the host executes that plugin tool through the extension runtime service
- THEN the plugin is invoked through the injected manager and the returned receipt records status and safe identifiers without raw plugin arguments, raw plugin output, credentials, headers, or environment values

### Requirement: Host-injected provider router execution
Clankers SHALL allow a host to execute provider/router requests through an explicitly injected runtime extension service instead of requiring ambient daemon/router/provider discovery.

#### Scenario: Embedded runtime fails closed without injected provider router
- **WHEN** an embedded/default-safe runtime provider-router execution is requested without an injected provider/router service
- **THEN** the request fails closed before daemon autostart, OAuth/login verifier writes, credential refresh persistence, or provider network execution can occur

#### Scenario: Desktop adapter executes through injected provider router
- **WHEN** the desktop runtime services are constructed with an explicit provider/router implementation
- **THEN** provider execution is routed through that implementation
- **AND** the receipt includes only sanitized status, safe identifiers, and aggregate stream/event counts
- **AND** the receipt excludes raw prompts, provider request bodies, model output, headers, tokens, environment values, and credentials
