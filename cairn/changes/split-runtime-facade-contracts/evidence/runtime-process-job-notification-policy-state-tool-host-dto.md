Artifact-Type: validation-log
Task-ID: I39,V38
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved process/job notification policy state and pure notification decision evaluation to the neutral tool-host process/job owner:

- Added `clankers_tool_host::process_jobs::ProcessJobNotificationPolicyState` with its rate-limit/suppression/completion-one-shot evaluation helper.
- Re-exported the state through `clankers-runtime::process_jobs` so existing runtime public API paths remain available.
- Kept the runtime-owned async `ProcessJobNotificationPolicyEngine` trait, default engine adapter, persistence/delivery orchestration, runtime error handling, and store/sink traits in `clankers-runtime`; the default engine now delegates to the neutral pure state evaluator.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clankers-tool-host -p clankers-runtime -p clankers --tests
cargo test -p clankers-tool-host notification_policy_state_delivers_completion_once_and_redacts_watch_patterns --lib
cargo test -p clankers-runtime default_notification_policy_delivers_completion_once --lib
cargo test -p clankers-runtime notification_decisions_and_persistence_redact_secret_excerpts --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
git diff --check
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
