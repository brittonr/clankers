## Phase 1: Catalog foundation

- [x] [serial] Write the tool catalog and capability-pack OpenSpec package. [covers=tool-host-embedding.catalog-builder] [evidence=openspec validate extract-tool-catalog-capability-packs --strict]
- [ ] [serial] Extract a reusable catalog builder from mode-specific tool registration. [covers=tool-host-embedding.catalog-builder]
- [ ] [parallel] Define capability packs, default embedding policy, and side-effect metadata. [covers=tool-host-embedding.capability-packs]
- [ ] [parallel] Add host custom-tool registration and collision policy. [covers=tool-host-embedding.custom-tools]

## Phase 2: Parity and docs

- [ ] [serial] Add default Clankers publication parity tests against existing built-in/plugin/MCP/gateway behavior. [covers=tool-host-embedding.catalog-builder.default-parity]
- [ ] [parallel] Add negative tests proving dangerous packs are absent unless explicitly enabled. [covers=tool-host-embedding.capability-packs.dangerous-opt-in]
- [ ] [parallel] Document embedding-safe tool profiles and pack prerequisites. [covers=tool-host-embedding.capability-packs]
