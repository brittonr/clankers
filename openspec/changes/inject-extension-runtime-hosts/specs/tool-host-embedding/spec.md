## MODIFIED Requirements

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
