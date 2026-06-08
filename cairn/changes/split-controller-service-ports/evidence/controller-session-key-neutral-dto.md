Artifact-Type: validation-log
Task-ID: I11,V10
Covers: r[remaining-coupling-drain.controller-service-ports.projection-owners], r[remaining-coupling-drain.controller-service-ports.behavior-validation], r[remaining-coupling-drain.controller-service-ports.closeout]
Status: pass

## Scope

Moved controller transport session-key indexing to neutral message contracts while keeping daemon protocol construction at the adapter edge.

- `clanker_message::SessionKey` now owns the iroh/Matrix session identity data shape and deterministic helpers.
- `clankers-protocol` re-exports the neutral key for stable wire/API compatibility.
- `clankers-controller::transport` now uses `clanker_message::SessionKey` for its key index and lookup/registration APIs, leaving protocol-specific constructors in the existing transport conversion module.

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
nix run .#cairn -- gate tasks split-controller-service-ports --root .
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
