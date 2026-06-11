Artifact-Type: validation-log
Task-ID: I1,I2,I3,V1,V2
Covers: r[ucan-basalt-daemon-auth.remote-auth-ux-docs.reference-guide], r[ucan-basalt-daemon-auth.remote-auth-ux-docs.entrypoints], r[ucan-basalt-daemon-auth.remote-auth-ux-docs.basalt-source], r[ucan-basalt-daemon-auth.remote-auth-ux-docs.contract-rail], r[ucan-basalt-daemon-auth.remote-auth-ux-docs.closeout]
Status: pass

## Scope

Added remote auth operator documentation for public UCAN + Basalt remote daemon admission.

## Documentation changes

- Added `docs/src/reference/remote-auth.md` with workflows for public UCAN credential creation, parent delegation, remote attach, chat/RPC and Matrix credential use, revocation/rotation, Basalt policy admission, redacted receipts, and the Basalt source boundary.
- Linked the reference from `docs/src/SUMMARY.md`, `docs/src/getting-started/auth.md`, `docs/src/reference/daemon.md`, and `README.md`.
- Updated token examples to target remote daemon audiences with `--for <REMOTE_IROH_PUBLIC_KEY>` and to avoid raw token/key material.

## Contract test

Added `tests/remote_auth_docs.rs` to assert that:

- documented token/attach examples use real clap flags such as `--read-only`, `--tools`, `--expire`, `--for`, `--from`, `--bot-commands`, `--session-manage`, `--delegate`, `--root`, and `--remote`;
- remote auth entrypoints mention public UCAN, Basalt policy, the legacy `clanker-auth` boundary, Matrix, chat/RPC, and redacted receipts;
- the remote auth reference does not embed raw token/key material or API-key fragments;
- the Basalt source-boundary docs match `Cargo.toml` and `flake.nix` (`../basalt`, `externalSources`, and `OnixResearch/basalt`).

## Validation

Commands run from repository root with `TMPDIR=/home/brittonr/.cargo-target/tmp` and `RUSTC_WRAPPER=`:

```text
cargo test -p clankers --test remote_auth_docs
cargo test -p clankers --test public_ucan_boundary
```

Outcomes:

- `remote_auth_docs`: 4 passed, 0 failed.
- `public_ucan_boundary`: 3 passed, 0 failed.

## Final lifecycle checks

Commands run after this evidence/task update:

```text
nix run .#cairn -- gate proposal remote-auth-ux-docs --root .
nix run .#cairn -- gate design remote-auth-ux-docs --root .
nix run .#cairn -- gate tasks remote-auth-ux-docs --root .
nix run .#cairn -- validate --root .
git diff --check
```

Outcomes:

- Proposal gate returned `"valid": true` and `"verdict": "PASS"`.
- Design gate returned `"valid": true` and `"verdict": "PASS"`.
- Tasks gate returned `"valid": true` and `"verdict": "PASS"`.
- Cairn validate returned `"valid": true` with 5 changes and 128 specs validated.
- `git diff --check` exited 0.
