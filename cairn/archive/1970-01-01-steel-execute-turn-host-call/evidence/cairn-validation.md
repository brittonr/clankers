Evidence-ID: cairn-validation
Artifact-Type: validation-log
Task-ID: V2
Covers: r[steel-execute-turn-host-call.verification.checker]
Status: pass

# Cairn and Repository Validation

## Cairn Gates

- `nix run .#cairn -- gate proposal steel-execute-turn-host-call --root .`
  - Result: PASS; no issues.
  - Receipt hash: `d95facc5757265b4361b83961a2f9878be8c981b543cce1a583156e052cf566f`.

- `nix run .#cairn -- gate design steel-execute-turn-host-call --root .`
  - Result: PASS; no issues.
  - Receipt hash: `a93dea5316029dcaed994aed15201b9d534a2742e34b160e4c29efe301eb43dd`.

- `nix run .#cairn -- validate --root .`
  - Result before task completion: pass; 1 active change, 49 specs validated, no change/spec issues.

- `nix run .#cairn -- gate tasks steel-execute-turn-host-call --root .`
  - Result after marking tasks complete and writing evidence: PASS; no issues.
  - Receipt hash: `1e7c374668b375bc9f14f1267ca9febea7bfd734361705d26ac775e108bee84e`.

- `nix run .#cairn -- validate --root .`
  - Result after task completion: pass; 1 active change, 49 specs validated, no change/spec issues.

- `nix run .#cairn -- sync steel-execute-turn-host-call --root . --execute`
  - Result: pass; wrote `cairn/specs/steel-execute-turn-host-call/spec.md`.
  - Sync receipt hash: `e19c981521d81c31f91498c5a29ac04af71bd7f565b869bef1bc6f1032bd77ca`.
  - Follow-up: manually carried the delta requirements into the canonical spec after sync produced the standard new-domain placeholder.

- `nix run .#cairn -- validate --root .`
  - Result after canonical spec update: pass; 1 active change, 50 specs validated, no change/spec issues.

- `nix run .#cairn -- archive steel-execute-turn-host-call --root . --execute`
  - Result: pass; moved the completed change to `cairn/archive/1970-01-01-steel-execute-turn-host-call`.
  - Archive receipt hash: `32af36cf21928eb67a7c1d3b001a6f425da6175bcb341a38f35f65e635600c52`.

- `nix run .#cairn -- validate --root .`
  - Result after archive: pass; 0 active changes, 49 specs validated, no issues.

## Repository Validation

- `TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= nix develop -c ./scripts/verify.sh`
  - Result: pass.
  - Verus: 71 verified, 0 errors.
  - No-std functional core validation bundle passed.
  - Controller FCIS/transport/client/parity nextest suite: 227 tests passed, 2 skipped.
  - Embedded controller parity nextest suite: 38 tests passed, 0 skipped.
  - Tracey: 47 of 47 requirements covered; 47 of 47 have verification references.
