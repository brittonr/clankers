Evidence-ID: cairn-validation
Artifact-Type: validation-log
Task-ID: V2
Covers: r[steel-host-call-json-payloads.verification.checker]
Status: pass

# Cairn and Repository Validation

## Cairn Gates

- `nix run .#cairn -- gate proposal steel-host-call-json-payloads --root .`
  - Result: PASS; no issues.
  - Receipt hash: `d7981f440025fee7431eb2eff9158f68c31938941e425ba659ecf2631ea542e7`.

- `nix run .#cairn -- gate design steel-host-call-json-payloads --root .`
  - Result: PASS; no issues.
  - Receipt hash: `8a8c296f6138a504328d6e4dcf4537090546b1987aa4b8a3d030ac367666d967`.

- `nix run .#cairn -- gate tasks steel-host-call-json-payloads --root .`
  - Result after marking tasks complete and writing evidence: PASS; no issues.
  - Receipt hash: `fa7b90db92218f5d9e32d0a5c1a0157f40eeee2ffe79a05c6857420817855d69`.

- `nix run .#cairn -- sync steel-host-call-json-payloads --root . --execute`
  - Result: pass; wrote `cairn/specs/steel-host-call-json-payloads/spec.md`.
  - Sync receipt hash: `148bc4789d547c1760579f6131ca51e05773cf9e5ba102d63eb4294a5ac18a43`.
  - Follow-up: manually carried the delta requirements into the canonical spec after sync produced the standard new-domain placeholder.

- `nix run .#cairn -- validate --root .`
  - Result after canonical spec update: pass; 1 active change, 51 specs validated, no change/spec issues.

- `nix run .#cairn -- archive steel-host-call-json-payloads --root . --execute`
  - Result: pass; moved the completed change to `cairn/archive/1970-01-01-steel-host-call-json-payloads`.
  - Archive receipt hash: `43da12bec0f0157fadddf4608b219788cf1924ebe59f59a42ee77d57d540deba`.

- `nix run .#cairn -- validate --root .`
  - Result after archive: pass; 0 active changes, 50 specs validated, no issues.

## Repository Validation

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c ./scripts/verify.sh`
  - Result: pass.
  - Verus: 71 verified, 0 errors.
  - No-std functional core validation bundle passed.
  - Controller FCIS/transport/client/parity nextest suite: 227 tests passed, 2 skipped.
  - Embedded controller parity nextest suite: 38 tests passed, 0 skipped.
  - Tracey: 47 of 47 requirements covered; 47 of 47 have verification references.
