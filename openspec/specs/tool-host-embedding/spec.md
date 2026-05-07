# tool-host-embedding Specification

## Purpose

Define the host-facing tool catalog and capability-pack boundary for embedded Clankers runtimes, including safe default profiles, explicit side-effect opt-ins, disabled-tool filtering, and host custom-tool registration.

## Requirements
### Requirement: Reusable tool catalog builder [r[tool-host-embedding.catalog-builder]]

The system MUST provide a reusable tool catalog builder that constructs Clankers tool registries outside CLI, TUI, daemon, ACP, or MCP mode modules and allows hosts to select built-in tool packs, disabled tools, optional runtimes, and custom tools explicitly.

#### Scenario: host builds a read-only catalog [r[tool-host-embedding.catalog-builder.readonly]]

- GIVEN an embedding host requests a read-only capability profile
- WHEN the host builds the tool catalog
- THEN the catalog includes only tools that cannot mutate files, spawn processes, send network requests, alter git state, start plugins, or deliver artifacts
- THEN mutating tools such as `write`, `edit`, `patch`, `bash`, `process`, `commit`, browser actions, plugin tools, MCP tools, and gateway delivery are absent unless separately enabled

#### Scenario: Clankers default catalog remains reproducible [r[tool-host-embedding.catalog-builder.default-parity]]

- GIVEN the normal Clankers CLI/TUI/daemon default policy
- WHEN the builder constructs the default Clankers catalog
- THEN the published built-in tools, tiers, optional tools, plugin tools, MCP tools, disabled-tool filtering, and gateway policy match the existing user-visible default surface

### Requirement: Capability packs [r[tool-host-embedding.capability-packs]]

The tool catalog MUST expose named capability packs for coarse host policy selection, and each pack MUST document its side-effect class, prerequisites, and default embedding status.

#### Scenario: dangerous packs are opt-in [r[tool-host-embedding.capability-packs.dangerous-opt-in]]

- GIVEN an embedding host uses default-safe runtime construction
- WHEN the tool catalog is built
- THEN shell/process execution, filesystem mutation, git mutation, browser automation, plugin process startup, MCP external servers, Matrix actions, and gateway delivery are not published by default
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
