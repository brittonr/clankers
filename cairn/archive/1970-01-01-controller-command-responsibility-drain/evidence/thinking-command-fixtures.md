# Thinking command extraction evidence

Evidence-ID: controller-thinking-command-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: controller-command-responsibility-drain.single-purpose-module,controller-command-responsibility-drain.verification
Date: 2026-06-02
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller command_thinking
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-controller command_responsibility
```

## Relevant output

```text
cargo nextest run -p clankers-controller command_thinking
PASS clankers-controller command_thinking::tests::parser_uses_core_levels_without_tui_dto
PASS clankers-controller command_thinking::tests::set_thinking_input_preserves_invalid_label_for_reducer_error
Summary: 2 tests run: 2 passed, 247 skipped

cargo nextest run -p clankers-controller command_responsibility
PASS clankers-controller command_responsibility::tests::command_responsibility_inventory_names_required_owners
PASS clankers-controller::fcis_shell_boundaries controller_command_responsibility_inventory_names_extracted_thinking_owner
Summary: 2 tests run: 2 passed, 247 skipped
```

## Coverage notes

`crates/clankers-controller/src/command_thinking.rs` now owns thinking label parsing and thinking `CoreInput` construction. `command.rs` keeps dispatching `SessionCommand` variants and emitting existing daemon messages, while projection helpers stay in `convert.rs`.
