Evidence-ID: cairn-validation
Artifact-Type: validation-log
Task-ID: V2
Covers: r[steel-execute-turn-authority.verification.checker]
Status: pass

# Cairn and Repository Validation

## Cairn Gates

- `nix run .#cairn -- gate proposal steel-execute-turn-authority --root .`
  - Result: PASS; no issues.
  - Receipt hash: `3b8c8da97b10c208f0158b20d80e8f1f70fc4fd1e6b031b2e08fb2f5bcdef336`.

- `nix run .#cairn -- gate design steel-execute-turn-authority --root .`
  - Result: PASS; no issues.
  - Receipt hash: `436b70b46f2aee383e30d49f1e4c31a722969d6ae603b139e5e70d5b287aad91`.

- `nix run .#cairn -- gate tasks steel-execute-turn-authority --root .`
  - Result before task completion: PASS; no issues.
  - Receipt hash: `883dc23beb499a6142518ccdac8f47f4e2010eedf8103012a7cd9ebbc7ffae58`.

- `nix run .#cairn -- gate tasks steel-execute-turn-authority --root .`
  - Result after marking tasks complete and writing evidence: PASS; no issues.
  - Receipt hash: `09d3173d33066e06f2a23d92be2826412ca162404f9c6acb36c718f7179cf93f`.

- `nix run .#cairn -- validate --root .`
  - Result before archive: pass; 1 active change, 48 specs validated, no change/spec issues.

- `nix run .#cairn -- sync steel-execute-turn-authority --root . --execute`
  - Result: pass; wrote `cairn/specs/steel-execute-turn-authority/spec.md`.
  - Sync receipt hash: `1bc750377d81c406fc47bd84af0da685eb42edde0088a62083cf7999ad97b280`.
  - Follow-up: manually carried the delta requirements into the canonical spec after sync produced the standard new-domain placeholder.

- `nix run .#cairn -- validate --root .`
  - Result after canonical spec update: pass; 1 active change, 49 specs validated, no change/spec issues.

- `nix run .#cairn -- archive steel-execute-turn-authority --root . --execute`
  - Result: pass; moved the completed change to `cairn/archive/1970-01-01-steel-execute-turn-authority`.
  - Archive receipt hash: `cb319bb516bd39c267643a41a3056e0886bd2956708b1d8acabe485eb21022d0`.

- `nix run .#cairn -- validate --root .`
  - Result after archive: pass; 0 active changes, 48 specs validated, no issues.

## Repository Validation

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c ./scripts/verify.sh`
  - Result: pass.
  - Verus: 71 verified, 0 errors.
  - No-std functional core validation bundle passed.
  - Controller FCIS/transport/client/parity nextest suite: 227 tests passed, 2 skipped.
  - Embedded controller parity nextest suite: 38 tests passed, 0 skipped.
  - Tracey: 47 of 47 requirements covered; 47 of 47 have verification references.
