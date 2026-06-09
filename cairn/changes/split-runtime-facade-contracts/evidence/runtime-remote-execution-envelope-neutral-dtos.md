Artifact-Type: validation-log
Task-ID: I53,V52,I54,V53
Covers: r[remaining-coupling-drain.runtime-facade-contract-split.green-contracts], r[remaining-coupling-drain.runtime-facade-contract-split.docs], r[remaining-coupling-drain.runtime-facade-contract-split.validation], r[remaining-coupling-drain.runtime-facade-contract-split.closeout]
Status: pass

## Scope

Moved reusable remote-execution dependency/envelope/failure DTOs to neutral message contracts:

- Added `clanker_message::{RemoteExecutionDependency, RemoteExecutionRequest, RemoteArtifactEnvelope, RemoteDependencyFailure, REMOTE_EXECUTION_ARTIFACT_SCHEMA_VERSION}` and a direct green dependency on `clankers-artifacts` for `ArtifactHash` / `RedactionClass` fields.
- Re-exported those DTOs through `clankers-runtime::effects` and the runtime crate root so existing runtime API paths remain available.
- Kept remote dependency sync evaluation, report-to-effect projection, and fail-closed handler policy in `clankers-runtime`.
- Regenerated `docs/src/generated/runtime-facade-api.md` after the ownership change.

## Validation

Commands run from repository root:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --lib remote_execution_dependency_and_envelope_contracts_are_stable
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime --lib effects::tests::
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check --tests -p clanker-message -p clankers-runtime -p clankers-agent -p clankers
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs --write-inventory
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-runtime-facade-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-message-contract-boundary.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-workspace-layering-rails.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= scripts/check-lego-architecture-boundaries.rs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-controller --test fcis_shell_boundaries
nix run .#cairn -- gate tasks split-runtime-facade-contracts --root .
nix run .#cairn -- validate --root .
git diff --check
```

All listed commands exited 0.
