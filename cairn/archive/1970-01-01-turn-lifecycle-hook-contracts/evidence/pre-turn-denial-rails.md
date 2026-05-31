Evidence-ID: pre-turn-denial-rails
Task-ID: V2
Artifact-Type: command-log
Covers: turn-lifecycle-hooks.validation, turn-lifecycle-hooks.dispatch-ownership
Status: passed

# Pre-turn Denial Rails

Implemented runtime seam coverage for the blocking `PreTurn` gate:

- Standalone `clankers-agent` test `pre_turn_deny_stops_before_model_request_and_posts_denied_outcome` proves a `PreTurn` denial runs after the user prompt is appended, prevents provider/model requests, emits one `PostTurn` denied outcome, and keeps turn tool-count metadata at zero.
- Controller-owned daemon path test `controller_owned_prompt_pre_turn_denial_prevents_provider_request` proves `SessionController::new(...)` wires the controller hook pipeline into the owned agent, then a `SessionCommand::Prompt` denied by `PreTurn` completes with `PromptDone` error and records no provider request.

## Commands

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-agent
```

Result: 189 tests run, 189 passed, 0 skipped.

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-controller controller_owned_prompt_pre_turn_denial
```

Result: 1 test run, 1 passed, 229 skipped.
