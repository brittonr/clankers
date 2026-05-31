Evidence-ID: closeout-validation
Artifact-Type: validation-log
Task-ID: V3
Covers: r[ucan-basalt-daemon-auth.verification.canonical-file-path-closeout]
Status: pass

# Closeout Validation

## Cairn Gates

- `nix run .#cairn -- gate proposal ucan-canonical-file-tool-paths --root .`
  - Result: PASS; receipt hash `62f4aa611df93fb301c4910345843cc61806d95ef846f8b5b70b9e1b44f4d749`.

- `nix run .#cairn -- gate design ucan-canonical-file-tool-paths --root .`
  - Result: PASS; receipt hash `e2daf9f33eb10ccde3df96c1affe07ec8155481dfe3671842526a8f91980dc0e`.

- `nix run .#cairn -- gate tasks ucan-canonical-file-tool-paths --root .`
  - Result after final V3 checkbox and evidence: PASS; receipt hash `e65b0e9f49f1ccf6c79695ef27d5d9b0ac3a9475feb9af0b7a180a08cc57f1dc`.

## Sync and Validation

- `nix run .#cairn -- sync ucan-canonical-file-tool-paths --root . --execute`
  - Result: PASS; receipt hash `2668dba0081c2f9de1864fb22d07fc4551bd4a68edd602527de8356544b008c3`.
  - Follow-up: manually verified and merged the canonical file path requirements into `cairn/specs/ucan-basalt-daemon-auth/spec.md` because the sync mutation manifest showed identical before/after hashes.

- `nix run .#cairn -- validate --root .`
  - Result after canonical spec update: PASS; 1 active change, 51 specs validated, no change/spec issues.

- `nix run .#cairn -- archive ucan-canonical-file-tool-paths --root . --execute`
  - Result: PASS; moved the completed change to `cairn/archive/1970-01-01-ucan-canonical-file-tool-paths`; receipt hash `b9f26396f7c623f66531d59eda3c24ec0b2a96ac4f0bc6d1c4aac4fce921c2f0`.

- `nix run .#cairn -- validate --root .`
  - Result after archive: PASS; 0 active changes, 50 specs validated, no change/spec issues.

## Repository Diff Check

- `git diff --check`
  - Result before and after archive: PASS; no whitespace errors.

## Post-Archive Checker

- `./scripts/check-ucan-canonical-file-tool-paths.rs`
  - Result after archive: PASS; checker read the canonical spec and archived tasks path and wrote `target/ucan-canonical-file-tool-paths/receipt.json`.
