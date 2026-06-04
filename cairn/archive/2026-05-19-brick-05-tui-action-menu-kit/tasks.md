## Phase 1: Contract and fixture shape

- [x] [serial] [covers=tui-action-menu-composition.tui-action-menu-kit.boundary] [evidence=openspec validate brick-05-tui-action-menu-kit --strict --json] Finalize the proposal, design, and delta spec for `tui-action-menu-kit`.
- [x] [serial] [covers=tui-action-menu-composition.tui-action-menu-kit.boundary] [evidence=source anchor readback] Identified the minimal anchors as `crates/clanker-tui-types/src/actions.rs`, `crates/clanker-tui-types/src/menu.rs`, and `crates/clankers-tui/src/components/leader_menu/mod.rs`; drained as a focused fixture plus source/docs/OpenSpec drift rail, not a new green SDK API.

## Phase 2: Implementation evidence

- [x] [serial] [covers=tui-action-menu-composition.tui-action-menu-kit.evidence] [evidence=cargo test -p clankers-tui tui_action_menu_kit_validates_typed_actions_conflicts_and_hide_rules] Added the focused positive fixture for typed `Action` parsing and leader-menu dispatch.
- [x] [parallel] [covers=tui-action-menu-composition.tui-action-menu-kit.evidence] [evidence=cargo test -p clankers-tui tui_action_menu_kit_validates_typed_actions_conflicts_and_hide_rules] Added deterministic priority conflict assertions and hidden-menu / unknown-action fail-closed assertions.
- [x] [parallel] [covers=tui-action-menu-composition.tui-action-menu-kit.drift] [evidence=./scripts/check-tui-action-menu-kit.rs] Added the source/docs/spec drift rail and documented the brick in `docs/src/reference/commands.md`.

## Phase 3: Validation and archive

- [x] [depends:implementation] [covers=tui-action-menu-composition.tui-action-menu-kit.evidence] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-tui tui_action_menu_kit_validates_typed_actions_conflicts_and_hide_rules] Run the focused verification for `tui-action-menu-kit` and capture the command in the archive note.
- [x] [depends:implementation] [covers=tui-action-menu-composition.tui-action-menu-kit.drift] [evidence=TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo fmt --check && git diff --check] Run formatting and whitespace checks.
- [x] [depends:implementation] [covers=tui-action-menu-composition.tui-action-menu-kit.boundary] [evidence=openspec validate tui-action-menu-composition --strict --json] Promote the spec delta, validate the canonical spec, and archive the change when complete.

Completed: 2026-05-19T02:46:15Z
