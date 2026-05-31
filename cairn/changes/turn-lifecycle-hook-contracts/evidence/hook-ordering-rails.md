Evidence-ID: hook-ordering-rails
Task-ID: V3
Artifact-Type: command-log
Covers: turn-lifecycle-hooks.validation
Status: complete

# Hook Ordering Rails

Implemented `controller_owned_prompt_hooks_lifecycle_notifications_and_tool_hooks_fire_in_order` in `crates/clankers-controller/src/command.rs`.

The rail drives the controller-owned prompt path with streaming event draining, a shared hook pipeline, a provider that performs a tool round-trip, and a real tool. It records the hook/lifecycle sequence:

1. `PrePrompt`
2. `PreTurn`
3. `TurnStart`
4. `PreTool`
5. `PostTool`
6. `TurnEnd`
7. `TurnStart`
8. `TurnEnd`
9. `PostTurn`
10. `PostPrompt`

The two `TurnStart`/`TurnEnd` pairs are the existing transcript/model-turn lifecycle notifications around the tool-use model turn and final text model turn; `PostTurn` and `PostPrompt` remain prompt-level hooks and fire once.

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-controller \
  controller_owned_prompt_hooks_lifecycle_notifications_and_tool_hooks_fire_in_order
```

Result:

```text
PASS [0.004s] clankers-controller command::tests::controller_owned_prompt_hooks_lifecycle_notifications_and_tool_hooks_fire_in_order
Summary: 1 test run: 1 passed, 230 skipped
```

```text
TMPDIR=/home/brittonr/.cargo-target/tmp rustfmt --check crates/clankers-controller/src/command.rs
```

Result: exit status 0.
