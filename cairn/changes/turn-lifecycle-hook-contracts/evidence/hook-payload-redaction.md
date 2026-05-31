Evidence-ID: hook-payload-redaction
Task-ID: V4
Artifact-Type: command-log
Covers: turn-lifecycle-hooks.payload-contract, turn-lifecycle-hooks.validation
Status: complete

# Hook Payload Redaction

Implemented script and plugin payload rails for safe turn payloads and lifecycle correlation.

## Covered rails

- `clankers-hooks` script rail: `script_payloads_preserve_prompt_turn_correlation_and_redact_safe_turn_fields`
  - Captures `PrePrompt` and `PostTurn` script stdin payloads.
  - Asserts both payloads share `prompt_id` and `prompt_digest`.
  - Uses a secret-like prompt (`token=...`) and secret-like tool-output error fixture (`sk-...`).
  - Asserts the `PostTurn` payload contains safe turn fields (`model`, counts, usage, status) but no raw `text` or `system_prompt`.
  - Asserts the serialized safe turn payload does not contain the raw prompt secret or tool-output secret.
- `clankers-plugin` plugin rail: `plugin_event_payloads_preserve_correlation_and_redact_safe_turn_fields`
  - Exercises the plugin hook event-envelope builder used by `PluginHookHandler`.
  - Asserts `PrePrompt` maps to `user_input` and `PostTurn` maps to `post_turn`.
  - Asserts the plugin event data preserves prompt/turn correlation and redacts the same secret-like prompt/tool-output fixture from safe turn fields.

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-hooks \
  script_payloads_preserve_prompt_turn_correlation_and_redact_safe_turn_fields
```

Result:

```text
PASS [0.010s] clankers-hooks script::tests::script_payloads_preserve_prompt_turn_correlation_and_redact_safe_turn_fields
Summary: 1 test run: 1 passed, 49 skipped
```

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-plugin \
  plugin_event_payloads_preserve_correlation_and_redact_safe_turn_fields
```

Result:

```text
PASS [0.003s] clankers-plugin hooks::tests::plugin_event_payloads_preserve_correlation_and_redact_safe_turn_fields
Summary: 1 test run: 1 passed, 41 skipped
```

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  rustfmt --check crates/clankers-hooks/src/script.rs crates/clankers-plugin/src/hooks.rs
```

Result: exit status 0.
