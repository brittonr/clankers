## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own SOUL Personality System. ✅ completed: 2026-05-01T23:59:00Z
  - Evidence: `openspec/changes/add-soul-personality-system/evidence/module-inventory.md` maps prompt assembly ownership (`crates/clankers-agent/src/system_prompt.rs`), runtime prompt mutation seams, CLI/tool surfaces, daemon/TUI/session paths, config paths, and safe metadata boundaries for a first-pass local policy module.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-02T00:01:00Z
  - Evidence: `openspec/changes/add-soul-personality-system/evidence/api-surface.md` defines `clankers soul status|validate`, a Specialty `soul_personality` status/validate tool, no required first-pass config, local SOUL file/discovery and safe preset-name validation, and explicit unsupported cases for remote/cloud/persona fetches, shell commands, raw prompt persistence, unsafe names, and live prompt mutation before a dedicated prompt-composition seam.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-02T00:00:43Z
  - Evidence: added `src/soul_personality.rs` first-pass policy helpers and tests for SOUL discovery/local file parsing, safe remote/command source kind parsing, personality-name validation, unsupported remote policy, and replay-safe error metadata. Verification passed `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers soul_personality --no-fail-fast` (4 passed).

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for SOUL Personality System.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
