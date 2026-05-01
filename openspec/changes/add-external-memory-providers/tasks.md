## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own External Memory Providers. ✅ completed: 2026-05-01T02:30:24Z
  - Evidence: `openspec/changes/add-external-memory-providers/evidence/module-inventory.md` maps existing local memory, config, tool publication, prompt/session integration, metadata, and test ownership boundaries.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T02:31:39Z
  - Evidence: `openspec/changes/add-external-memory-providers/evidence/api-surface.md` defines disabled-by-default `externalMemory` config, `external_memory` tool actions, TUI/slash UX, metadata contract, and unsupported cases.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T02:35:11Z
  - Evidence: added `ExternalMemorySettings` parsing/validation tests covering disabled defaults, valid local config, blank policy fields, explicit HTTP unsupported errors, and deep-merge behavior. Verified with `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers-config external_memory --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers-config`, and `git diff --check`.

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for External Memory Providers. ✅ completed: 2026-05-01T02:44:55Z
  - Evidence: added `src/tools/external_memory.rs` with disabled-by-default publication gating, local-provider search/status actions, bounded result handling, safe provider metadata, remote-provider unsupported errors before contact, and unit tests. Verified with `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers external_memory --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers`, and `git diff --check`.
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T02:47:00Z
  - Evidence: `src/modes/common.rs` publishes `external_memory` as a Specialty tool only when `settings.externalMemory.enabled` validates. This shared `ToolEnv` path is used by standalone prompt, interactive TUI rebuilds, and daemon/session tool construction; tests prove disabled, enabled, and invalid-config publication behavior.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
