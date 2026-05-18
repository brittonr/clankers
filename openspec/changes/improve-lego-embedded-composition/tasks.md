## Phase 1: Adapter bricks

- [ ] [serial] Add a shell-free `clankers-adapters` crate with dependency denylist coverage. [covers=r[embedded-composition-kits.adapter-bricks.shell-free]] [evidence=`scripts/check-embedded-agent-sdk.sh`]
- [ ] [parallel] Implement deterministic event, cancellation, retry-sleeper, usage-observer, and fake/scripted model adapter bricks with focused tests. [covers=r[embedded-composition-kits.adapter-bricks.common-seams]] [evidence=`cargo test -p clankers-adapters`]
- [ ] [parallel] Add replaceability tests showing app-owned adapters can replace reusable bricks without engine changes. [covers=r[embedded-composition-kits.adapter-bricks.replaceable]] [evidence=`cargo test -p clankers-adapters replaceable`]

## Phase 2: Composition kits and recipes

- [ ] [depends:adapter-bricks] Add a minimal embedded composition kit or recipe that runs outside the workspace graph without shell/runtime dependencies. [covers=r[embedded-composition-kits.product-kits.minimal]] [evidence=`cargo run --locked --manifest-path examples/embedded-minimal-kit/Cargo.toml`]
- [ ] [depends:adapter-bricks] Add a tool-enabled kit or recipe covering successful tool, missing tool, tool error, capability denial, and truncation paths. [covers=r[embedded-composition-kits.product-kits.tool-enabled]] [evidence=`cargo run --locked --manifest-path examples/embedded-tool-kit/Cargo.toml`]
- [ ] [parallel] Update docs to label daemon/MCP/ACP as app-edge integration surfaces, not generic SDK dependencies. [covers=r[embedded-composition-kits.product-kits.daemon-edge]] [evidence=`scripts/check-embedded-agent-sdk.sh`]

## Phase 3: Declarative tool catalogs

- [ ] [serial] Define typed embedded tool-catalog DTOs and metadata conversion into `ToolCatalog`-compatible data without runtime startup. [covers=r[embedded-composition-kits.tool-catalogs.metadata]] [evidence=`cargo test tool_catalog_metadata`]
- [ ] [parallel] Add fail-closed validation for duplicate names, schema errors, unknown runtime kinds, unsafe defaults, and unsupported approval/redaction policy. [covers=r[embedded-composition-kits.tool-catalogs.fail-closed]] [evidence=`cargo test tool_catalog_validation`]
- [ ] [parallel] Document supported serialized catalog formats and keep public semantics parser-neutral. [covers=r[embedded-composition-kits.tool-catalogs.implementation-neutral]] [evidence=`scripts/check-embedded-agent-sdk.sh`]

## Phase 4: Capability packs

- [ ] [serial] Add named embedded capability-pack presets with exact set or generated snapshot tests. [covers=r[embedded-composition-kits.capability-packs.no-expansion]] [evidence=`cargo test capability_pack`]
- [ ] [parallel] Add docs/API affordances that mark mutating, shell, network, raw-log, and secret-adjacent packs as explicit opt-ins. [covers=r[embedded-composition-kits.capability-packs.explicit-danger]] [evidence=`scripts/check-embedded-agent-sdk.sh`]

## Phase 5: Acceptance rail and crate guidance

- [ ] [serial] Extend `scripts/check-embedded-agent-sdk.sh` to run new adapter, kit, catalog, capability-pack, dependency-denylist, and recipe checks. [covers=r[embedded-composition-kits.acceptance-rail.one-command]] [evidence=`scripts/check-embedded-agent-sdk.sh`]
- [ ] [parallel] Add checked green/yellow/red crate guidance for product embedding docs. [covers=r[embedded-composition-kits.recipes.crate-guidance]] [evidence=`scripts/check-embedded-agent-sdk.sh`]
- [ ] [parallel] Ensure executable recipes cover minimal, tool-enabled, and negative/fail-closed composition paths. [covers=r[embedded-composition-kits.recipes.coverage]] [evidence=`scripts/check-embedded-agent-sdk.sh`]

## Phase 6: Final verification

- [ ] [serial] Run focused OpenSpec validation for this change. [covers=r[embedded-composition-kits.acceptance-rail.one-command]] [evidence=`openspec validate improve-lego-embedded-composition --strict --json`]
- [ ] [serial] Run formatting/diff checks and the embedded acceptance rail before implementation landing. [covers=r[embedded-composition-kits.acceptance-rail.one-command]] [evidence=`cargo fmt --check`, `git diff --check`, `scripts/check-embedded-agent-sdk.sh`]
