Artifact-Type: validation-log
Task-ID: I24,V23
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved confirmation action/decision DTOs to neutral message contracts:

- Added `clanker_message::ConfirmationAction` for host confirmation action labels.
- Added `clanker_message::ConfirmationDecision` for approved/denied host decisions, preserving helper constructors and secret-marker redaction.
- Re-exported those DTOs through `clankers-runtime::confirmation` / crate root so existing runtime public API paths remain available.
- Kept `ConfirmationRequest`, confirmation brokers, runtime timeout/cancellation behavior, event metadata, and executable confirmation flow in `clankers-runtime`; only reusable action/decision record ownership moved.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-runtime -p clankers --tests
cargo test -p clanker-message confirmation_action_custom_roundtrip_preserves_payload --lib
cargo test -p clanker-message confirmation_decision_helpers_sanitize_secret_reasons --lib
cargo test -p clankers-runtime confirmation_broker_fail_closed_for_absent_timeout_cancelled --lib
cargo test -p clankers-runtime confirmation_request_metadata_redacts_secret_markers --lib
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
