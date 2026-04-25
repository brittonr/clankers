Task-ID: V4b1,V4b2
Covers: embeddable-agent-engine.agent-default-assembly, embeddable-agent-engine.host-adapter-parity
Artifact-Type: validation-evidence

# V4b entrypoint routing evidence

## Test paths

- Standalone interactive:
  - `src/modes/agent_task.rs::tests::handle_prompt_routes_standalone_agent_through_prompt_path`
- Daemon session/controller:
  - `crates/clankers-controller/src/command.rs::tests::test_handle_command_prompt_uses_reducer_start_effect_and_preserves_shell_events`
- Attach client:
  - `src/modes/attach.rs::tests::attach_regular_prompt_routes_to_daemon_session_prompt`

## Commands

- `cargo test --lib handle_prompt_routes_standalone_agent_through_prompt_path`: PASS.
- `cargo test --lib attach_regular_prompt_routes_to_daemon_session_prompt`: PASS.
- `cargo test -p clankers-controller --lib test_handle_command_prompt_uses_reducer_start_effect_and_preserves_shell_events`: PASS.

## Result

Standalone interactive prompt handling calls `Agent::prompt`, which now routes through `run_turn_loop` and the host runner. Daemon session prompt handling accepts `SessionCommand::Prompt`, applies core start effects, calls the shared agent prompt path, and emits prompt completion. Attach mode forwards regular user input as `SessionCommand::Prompt`, so attach prompts reach the same daemon/session controller path.
