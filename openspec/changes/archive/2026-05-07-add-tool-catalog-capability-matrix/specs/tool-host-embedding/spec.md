## ADDED Requirements

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
