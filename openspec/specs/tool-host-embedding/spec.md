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

### Requirement: Auth Runtime Extension Access [r[tool-host-embedding.auth-runtime-extension-access]]

The system MUST route embedded auth-store lookup and credential-pool selection through explicit runtime extension services when those services are used by an embedding host.

#### Scenario: Default-safe auth services [r[tool-host-embedding.auth-runtime-extension-access.default-safe]]

- GIVEN an embedded/default runtime without injected auth services
- WHEN auth-store lookup, pending login verifier access, refresh persistence, or credential-pool selection is requested
- THEN the operation MUST fail closed without reading auth files, writing verifier state, refreshing tokens, or persisting credentials

#### Scenario: Injected auth lookup receipt [r[tool-host-embedding.auth-runtime-extension-access.injected-lookup]]

- GIVEN a host-injected auth-store snapshot with provider/account entries
- WHEN a runtime auth lookup is requested
- THEN the service MUST return a safe receipt containing provider/account/status/count/kind metadata and MUST NOT include credential values, refresh tokens, verifier contents, headers, environment values, or raw auth-file contents

#### Scenario: Injected credential-pool selection receipt [r[tool-host-embedding.auth-runtime-extension-access.pool-selection]]

- GIVEN a host-injected auth-store snapshot and credential-pool strategy request
- WHEN runtime credential-pool selection is requested
- THEN the service MUST select from injected entries using safe provider/account/strategy metadata and MUST NOT start OAuth flows, refresh credentials, or expose credential values

### Requirement: Tool catalog capability matrix coverage [r[tool-host-embedding.catalog-capability-matrix]]
The system MUST verify tool catalog construction across an explicit matrix of capability packs, disabled filters, custom tools, collision policies, extension runtime availability, and side-effect classes.

#### Scenario: capability packs compose without implicit dangerous expansion [r[tool-host-embedding.catalog-capability-matrix.pack-composition]]
- GIVEN a matrix case enables one or more capability packs
- WHEN the catalog builder constructs descriptors
- THEN only tools from explicitly enabled packs are present
- THEN enabling one dangerous pack does not publish unrelated dangerous packs

#### Scenario: disabled filters override enabled packs [r[tool-host-embedding.catalog-capability-matrix.disabled-overrides]]
- GIVEN a matrix case enables a pack and disables a tool name from that pack
- WHEN the catalog is built
- THEN the disabled tool is omitted
- THEN safe metadata explains the omission without leaking credentials, raw inputs, or environment values

#### Scenario: host custom tools exercise collision policy [r[tool-host-embedding.catalog-capability-matrix.custom-collision]]
- GIVEN a matrix case registers a host custom tool whose name collides or does not collide with a Clankers tool
- WHEN the catalog is built
- THEN the configured collision policy is enforced deterministically
- THEN successful host tools retain source labels and side-effect metadata

#### Scenario: metadata queries do not start extension runtimes [r[tool-host-embedding.catalog-capability-matrix.no-eager-start]]
- GIVEN a matrix case with extension packs disabled or extension runtime absent
- WHEN catalog metadata is requested
- THEN no plugin, MCP, router, auth, or gateway service is started
- THEN unavailable extension-backed tools are omitted or marked unavailable according to host policy

### Requirement: Runtime extension service matrix coverage [r[tool-host-embedding.runtime-extension-service-matrix]]
The system MUST verify runtime extension services across an explicit matrix of absent, injected-success, injected-error, and denied states for auth, credential-pool, provider/router, plugin, and future extension placeholders where applicable.

#### Scenario: default-safe runtime fails closed independently [r[tool-host-embedding.runtime-extension-service-matrix.default-safe]]
- GIVEN a default-safe embedded runtime without injected extension services
- WHEN auth lookup, credential-pool selection, provider execution, plugin publication, or plugin execution is requested
- THEN each operation fails closed before hidden file reads, verifier writes, credential refresh persistence, daemon autostart, socket access, subprocess startup, or network provider execution

#### Scenario: mixed injected and absent services do not fall back ambiently [r[tool-host-embedding.runtime-extension-service-matrix.mixed-services]]
- GIVEN a matrix case injects only a subset of runtime extension services
- WHEN an operation for an injected service succeeds or fails
- THEN absent services are not discovered, started, or consulted implicitly
- THEN the result depends only on the explicitly injected service and request policy

#### Scenario: safe receipts are uniformly redacted [r[tool-host-embedding.runtime-extension-service-matrix.redaction]]
- GIVEN any runtime service matrix case returns a success, denial, or error receipt
- WHEN the receipt is serialized or logged for host inspection
- THEN it excludes raw prompts, provider request bodies, model output, credentials, refresh tokens, verifier contents, headers, environment values, raw auth files, raw plugin arguments, and raw plugin output
- THEN it includes only safe status, provider/account/tool identifiers, counts, and aggregate diagnostics

#### Scenario: side-effect sentinels prove negative claims [r[tool-host-embedding.runtime-extension-service-matrix.side-effect-sentinels]]
- GIVEN matrix tests install filesystem, socket, and fake-service counters before execution
- WHEN fail-closed or absent-service cases run
- THEN the test asserts that sentinels were not touched and fake services were not invoked outside the declared matrix state
