Task-ID: V6
Covers: embeddable-agent-engine.composable-host-contract, embeddable-agent-engine.reusable-tool-host, embeddable-agent-engine.reusable-stream-accumulator, embeddable-agent-engine.host-extraction-rails, embeddable-agent-engine.host-artifact-freshness
Artifact-Type: validation-evidence

# V6 final validation evidence

## Command bundle

Pueue task: `26` (`extract host final validation bundle`) — PASS.

Commands run:

- `cargo test -p clankers-engine-host --lib`: PASS (26 tests).
- `cargo test -p clankers-tool-host --lib`: PASS (10 tests).
- `cargo test -p clankers-agent --lib turn::`: PASS (54 tests).
- `cargo test -p clankers-controller --test fcis_shell_boundaries`: PASS (34 tests).
- `cargo test -p clankers-controller --lib core_engine_composition`: PASS (10 tests).
- `cargo test -p clankers-controller --lib auto_test`: PASS (32 tests).
- `cargo test -p clankers-core --lib`: PASS (41 tests).
- `cargo check -p clankers-agent`: PASS.
- `cargo test --lib handle_prompt_routes_standalone_agent_through_prompt_path`: PASS (1 test).
- `cargo test --lib attach_regular_prompt_routes_to_daemon_session_prompt`: PASS (1 test).
- `cargo test -p clankers-controller --lib test_handle_command_prompt_uses_reducer_start_effect_and_preserves_shell_events`: PASS (1 test).
- `./scripts/check-llm-contract-boundary.sh`: PASS.
- `unit2nix --workspace --force --no-check -o build-plan.json`: PASS (`Wrote build-plan.json`).
- `cargo xtask docs`: PASS (`docs built → docs/book/`).
- `openspec validate extract-composable-engine-host --strict`: PASS (`Change 'extract-composable-engine-host' is valid`).

## Notes

- `clankers-db` emitted the pre-existing `SearchIndex::schema` dead-code warning during dependent builds.
- `clankers-agent --lib turn::` emitted dead-code warnings for test-only legacy helper functions retained for reducer-contract tests.
- The final artifact freshness run updated generated docs stats/architecture/crate pages to reflect the new validation code.

## Result

Final validation covers host runner/tool host crates, migrated agent adapters, controller FCIS/source rails, composition/core reducer behavior, standalone/attach/daemon routing tests, cargo-tree/source dependency rails, generated artifact freshness, and OpenSpec strict validation.
