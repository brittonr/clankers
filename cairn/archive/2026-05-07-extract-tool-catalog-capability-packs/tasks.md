## Phase 1: Catalog foundation

- [x] [serial] Write the tool catalog and capability-pack OpenSpec package. [covers=tool-host-embedding.catalog-builder] [evidence=openspec validate extract-tool-catalog-capability-packs --strict]
- [x] [serial] Introduce a reusable catalog builder boundary for embeddable runtime registration. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.catalog-builder] [evidence=clankers_runtime::ToolCatalogBuilder]
- [x] [parallel] Define capability packs, default embedding policy, and side-effect metadata. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs] [evidence=clankers-runtime::tests::tool_catalog_embedding_safe_excludes_dangerous_packs]
- [x] [parallel] Add host custom-tool registration and collision policy. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.custom-tools] [evidence=clankers-runtime::tests::tool_catalog_supports_custom_tool_collision_policy]

## Phase 2: Parity and docs

- [x] [serial] Extract default Clankers publication from existing built-in/plugin/MCP/gateway registration and add parity tests against that source. ✅ (completed: 2026-05-07T03:28:16Z) [covers=tool-host-embedding.catalog-builder.default-parity] [evidence=src/modes/common.rs::tests::runtime_catalog_matches_existing_default_tool_registration]
- [x] [parallel] Add negative tests proving dangerous packs are absent unless explicitly enabled. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs.dangerous-opt-in] [evidence=clankers-runtime::tests::tool_catalog_embedding_safe_excludes_dangerous_packs]
- [x] [parallel] Add disabled-tool filtering to the catalog builder and tests proving disabled tools are omitted from host-visible metadata. ✅ (completed: 2026-05-07T02:54:44Z) [covers=tool-host-embedding.catalog-builder.disabled-tools] [evidence=clankers-runtime::tests::tool_catalog_filters_disabled_tools_from_host_metadata]
- [x] [parallel] Document embedding-safe tool profiles and pack prerequisites. ✅ 26m (started: 2026-05-07T02:22:19Z → completed: 2026-05-07T02:48:26Z) [covers=tool-host-embedding.capability-packs] [evidence=docs/src/reference/embedding.md]
