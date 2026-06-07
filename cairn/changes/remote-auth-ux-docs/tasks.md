# Tasks: Remote Auth UX Docs

## Phase 1: Documentation

- [ ] [serial] I1: Add a remote-auth reference guide covering public UCAN credential creation, delegation from a parent credential, remote attach/chat/Matrix credential use, revocation/rotation, Basalt policy admission, and redacted receipts. r[ucan-basalt-daemon-auth.remote-auth-ux-docs.reference-guide] [covers=ucan-basalt-daemon-auth.remote-auth-ux-docs.reference-guide]
- [ ] [serial] I2: Update README and getting-started/daemon docs to link the reference guide and distinguish public UCAN + Basalt remote auth from legacy local `clanker-auth` compatibility. r[ucan-basalt-daemon-auth.remote-auth-ux-docs.entrypoints] [covers=ucan-basalt-daemon-auth.remote-auth-ux-docs.entrypoints]
- [ ] [serial] I3: Document the Basalt source boundary: local Cargo uses `../basalt`, while Nix maps that path to the pinned `OnixResearch/basalt` input through flake `externalSources`. r[ucan-basalt-daemon-auth.remote-auth-ux-docs.basalt-source] [covers=ucan-basalt-daemon-auth.remote-auth-ux-docs.basalt-source]

## Phase 2: Verification

- [ ] [serial] V1: Add a deterministic docs/help contract test that checks remote-auth examples use real clap flags, mention public UCAN + Basalt, avoid legacy-token guidance for remote access, and do not embed raw token or key material. r[ucan-basalt-daemon-auth.remote-auth-ux-docs.contract-rail] [covers=ucan-basalt-daemon-auth.remote-auth-ux-docs.contract-rail]
- [ ] [serial] V2: Run the remote-auth docs contract test together with `tests/public_ucan_boundary.rs`, Cairn proposal/design/tasks gates, `nix run .#cairn -- validate --root .`, and `git diff --check` before closeout. r[ucan-basalt-daemon-auth.remote-auth-ux-docs.closeout] [covers=ucan-basalt-daemon-auth.remote-auth-ux-docs.closeout]
