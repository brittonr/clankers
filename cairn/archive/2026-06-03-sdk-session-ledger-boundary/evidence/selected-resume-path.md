Task-ID: I2
Covers: sdk-session-ledger-boundary.ledger-boundary.selected-path
Artifact-Type: implementation-evidence

# Selected Resume Path: Daemon SessionBuilder Seed Replay

## Selected path

The selected restore/resume path is the socketless daemon session builder path:

- `src/modes/daemon/session_builder.rs::load_recovery_seed_messages`
- `src/modes/daemon/session_builder.rs::resolve_session_resume_in_dir`
- `src/modes/daemon/session_builder.rs::serialize_seed_messages`

## Boundary move

`serialize_seed_messages` now delegates to `src/modes/session_ledger.rs::desktop_messages_to_serialized_seed_messages`.
That adapter projects desktop `AgentMessage` transcript records into neutral `clankers_runtime::SessionLedgerEntry` / `SessionLedgerMessage` DTOs before narrowing them to the current daemon seed protocol.

Desktop `clankers-session::SessionManager` remains the compatibility reader for existing automerge/JSONL session files, while the selected seed replay behavior consumes neutral ledger entries at the adapter seam.

## Validation

Focused tests/checks:

- `cargo nextest run -p clankers daemon::session_builder::tests::create_plan_resolves_resume_messages_without_socket`
- `cargo nextest run -p clankers modes::session_ledger::tests`
- `nix develop -c cargo -q -Zscript scripts/check-session-ledger-boundary.rs`
