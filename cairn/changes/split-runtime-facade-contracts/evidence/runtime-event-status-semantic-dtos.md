Artifact-Type: validation-log
Task-ID: I18,V17
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Reused neutral semantic message contracts for runtime event status DTOs:

- Replaced runtime-local `StopReason` with a compatibility re-export of `clanker_message::SemanticStopReason`.
- Replaced runtime-local `ToolStatus` with a compatibility re-export of `clanker_message::SemanticToolStatus`.
- Preserved `clankers_runtime::StopReason`, `clankers_runtime::ToolStatus`, and `clankers_runtime::events::{StopReason,ToolStatus}` public paths through re-exports.
- Kept runtime event emission, semantic projection helpers, session execution behavior, and host adapter behavior in `clankers-runtime`; only duplicate status enum ownership was removed.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message semantic_event_ordering_fixture_covers_core_behavior --lib
cargo test -p clankers-runtime runtime_facade_tool_feedback_uses_engine_host_turn_loop --lib
scripts/check-runtime-facade-boundary.rs --write-inventory
scripts/check-runtime-facade-boundary.rs
cargo -q -Zscript scripts/check-runtime-facade-split.rs
scripts/check-message-contract-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
cargo test -p clankers --no-run
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
