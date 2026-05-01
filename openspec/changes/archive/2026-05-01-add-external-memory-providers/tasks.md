## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own External Memory Providers. ✅ completed: 2026-05-01T02:30:24Z
  - Evidence: `openspec/changes/archive/2026-05-01-add-external-memory-providers/evidence/module-inventory.md` maps existing local memory, config, tool publication, prompt/session integration, metadata, and test ownership boundaries.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ✅ completed: 2026-05-01T02:31:39Z
  - Evidence: `openspec/changes/archive/2026-05-01-add-external-memory-providers/evidence/api-surface.md` defines disabled-by-default `externalMemory` config, `external_memory` tool actions, TUI/slash UX, metadata contract, and unsupported cases.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ✅ completed: 2026-05-01T02:35:11Z
  - Evidence: added `ExternalMemorySettings` parsing/validation tests covering disabled defaults, valid local config, blank policy fields, explicit HTTP unsupported errors, and deep-merge behavior. Verified with `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers-config external_memory --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers-config`, and `git diff --check`.

## Phase 2: Implementation

- [x] Implement the minimal backend or adapter for External Memory Providers. ✅ completed: 2026-05-01T02:44:55Z
  - Evidence: added `src/tools/external_memory.rs` with disabled-by-default publication gating, local-provider search/status actions, bounded result handling, safe provider metadata, remote-provider unsupported errors before contact, and unit tests. Verified with `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers external_memory --no-fail-fast`, `CARGO_TARGET_DIR=target cargo check --tests -p clankers`, and `git diff --check`.
- [x] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable. ✅ completed: 2026-05-01T02:47:00Z
  - Evidence: `src/modes/common.rs` publishes `external_memory` as a Specialty tool only when `settings.externalMemory.enabled` validates. This shared `ToolEnv` path is used by standalone prompt, interactive TUI rebuilds, and daemon/session tool construction; tests prove disabled, enabled, and invalid-config publication behavior.
- [x] Persist or log session metadata needed for replay and debugging. ✅ completed: 2026-05-01T02:49:08Z
  - Evidence: `external_memory` results attach safe `ToolResult.details` metadata (`source`, provider kind/name, action, status, elapsed time, result count, inject flag/error kind) which existing `ToolResultMessage.details` session persistence and tool-host outcomes carry through replay/debug paths. Added tests proving metadata omits raw queries, result text, credential env values, and redacts secret-like errors.

## Phase 3: Verification and Documentation

- [x] Add integration tests for the primary successful path and at least one failure path. ✅ completed: 2026-05-01T22:21:50Z
  - Evidence: added `tests/external_memory.rs` integration coverage for enabled local-provider search through shared Specialty publication, missing runtime database failure details, and disabled-config non-publication. Verified with `cargo fmt` and `CARGO_TARGET_DIR=target cargo nextest run -p clankers --test external_memory --no-fail-fast`.
- [x] Update README/docs and any relevant built-in tool or command lists. ✅ completed: 2026-05-01T22:30:00Z
  - Evidence: README Specialty tool list documents `external_memory` publication behind `externalMemory.enabled`; `docs/src/reference/config.md` includes an `externalMemory` settings example and first-pass local/HTTP/metadata behavior notes.
- [x] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`. ✅ completed: 2026-05-01T22:40:00Z
  - Evidence: final verification passed `cargo fmt`, `CARGO_TARGET_DIR=target cargo nextest run -p clankers-config external_memory --no-fail-fast` (5 passed), `CARGO_TARGET_DIR=target cargo nextest run -p clankers external_memory --no-fail-fast` (10 passed), `CARGO_TARGET_DIR=target cargo nextest run -p clankers --test external_memory --no-fail-fast` (3 passed), `CARGO_TARGET_DIR=target cargo check --tests -p clankers-config -p clankers`, and `git diff --check`; helper verification was rerun after marking this task complete.
