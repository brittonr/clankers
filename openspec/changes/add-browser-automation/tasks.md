## Phase 1: Discovery and API Shape

- [x] Inventory existing clankers modules that should own Browser Automation. ⏱ started: 2026-04-30T22:53:03Z, completed: 2026-04-30T22:56:27Z, elapsed: 3m24s. Evidence: `evidence/browser-module-inventory.md`.
- [x] Define the user-facing CLI/TUI/tool/config surface and document unsupported first-pass cases. ⏱ started: 2026-04-30T22:56:58Z, completed: 2026-04-30T22:57:53Z, elapsed: 55s. Evidence: `design.md`, `specs/browser-automation/spec.md`.
- [x] Add focused tests for parsing, configuration, and policy boundaries. ⏱ started: 2026-04-30T22:58:25Z, completed: 2026-04-30T23:06:03Z, elapsed: 7m38s. Evidence: `cargo fmt`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers-config browser_automation --no-fail-fast`; `CARGO_TARGET_DIR=target cargo nextest run -p clankers browser --no-fail-fast`; `CARGO_TARGET_DIR=target cargo check --tests -p clankers-config -p clankers`.

## Phase 2: Implementation

- [ ] Implement the minimal backend or adapter for Browser Automation.
- [ ] Wire the capability through standalone prompt, interactive TUI, and daemon/session paths where applicable.
- [ ] Persist or log session metadata needed for replay and debugging.

## Phase 3: Verification and Documentation

- [ ] Add integration tests for the primary successful path and at least one failure path.
- [ ] Update README/docs and any relevant built-in tool or command lists.
- [ ] Run `cargo fmt`, targeted `cargo nextest`, `cargo check --tests`, and `git diff --check`.
