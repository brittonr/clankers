## Phase 1: Catalog foundation

- [x] [serial] Write the tool catalog and capability-pack OpenSpec package. [covers=tool-host-embedding.catalog-builder] [evidence=openspec validate extract-tool-catalog-capability-packs --strict]
- [x] [serial] Extract a reusable catalog builder from mode-specific tool registration. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.catalog-builder] [evidence=clankers_runtime::ToolCatalogBuilder]
- [x] [parallel] Define capability packs, default embedding policy, and side-effect metadata. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs] [evidence=clankers-runtime::tests::tool_catalog_embedding_safe_excludes_dangerous_packs]
- [x] [parallel] Add host custom-tool registration and collision policy. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.custom-tools] [evidence=clankers-runtime::tests::tool_catalog_supports_custom_tool_collision_policy]

## Phase 2: Parity and docs

- [x] [serial] Add default Clankers publication parity tests against existing built-in/plugin/MCP/gateway behavior. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.catalog-builder.default-parity] [evidence=ToolCatalog::desktop_default and ToolCatalog::embedding_safe coverage in CARGO_TARGET_DIR=target cargo test -p clankers-runtime]
- [x] [parallel] Add negative tests proving dangerous packs are absent unless explicitly enabled. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs.dangerous-opt-in] [evidence=clankers-runtime::tests::tool_catalog_embedding_safe_excludes_dangerous_packs]
- [x] [parallel] Document embedding-safe tool profiles and pack prerequisites. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs] [evidence=docs/src/reference/embedding.md]
