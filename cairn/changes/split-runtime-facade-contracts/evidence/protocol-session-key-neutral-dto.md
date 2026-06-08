Artifact-Type: validation-log
Task-ID: I9,V8
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved daemon transport session identity DTO ownership to neutral message contracts:

- Added `clanker_message::SessionKey` with the existing iroh/Matrix variants, display formatting, deterministic directory-name helper, and Matrix room lookup.
- Re-exported `SessionKey` from `clankers-protocol::types` and crate root so existing daemon protocol APIs and serialized JSON remain stable.
- Kept protocol framing/request types in `clankers-protocol`; only reusable session identity data ownership moved to the neutral contract crate.

## Validation

Commands run from repository root:

```text
cargo check -p clanker-message -p clankers-protocol -p clankers-controller -p clankers --tests
cargo test -p clankers --no-run
cargo test -p clanker-message session_key_roundtrip_preserves_matrix_identity --lib
cargo test -p clanker-message session_key_matrix_dir_name_sanitizes --lib
cargo test -p clankers-protocol --lib
cargo test -p clankers-controller --test fcis_shell_boundaries
scripts/check-controller-runtime-boundary.rs
scripts/check-lego-architecture-boundaries.rs
scripts/check-workspace-layering-rails.rs
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
