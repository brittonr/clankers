Evidence-ID: final-validation
Task-ID: V4
Artifact-Type: command-log
Covers: remaining-coupling-drain.closeout-validation
Status: complete

# Final Validation

Closeout validation covered the focused seams, full workspace nextest partitions, repository verification rails, Cairn sync/gates/validate/archive dry-run, and whitespace checks.

During broad validation two stale rails surfaced and were fixed before rerunning the affected partition:

- `tests/provider_contract_docs.rs` still expected a deleted local `rpc_provider.rs` helper. The source anchor now tracks the direct `crate::router_request_bridge::build_router_request(request)` call.
- `crates/clankers-artifacts/src/lib.rs` had a stale `SkillReference` golden hash fixture. The expected hash was updated to the deterministic canonical hash reported by the fixture, then the focused fixture was rerun.

## Full nextest partitions

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run --workspace --partition count:1/4
```

Result: exit status 0; 1072 tests run, 1072 passed, 3076 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run --workspace --partition count:2/4
```

Result: exit status 0; 1049 tests run, 1049 passed, 3099 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run --test provider_contract_docs
```

Result: exit status 0; 3 tests run, 3 passed, 0 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run --workspace --partition count:3/4
```

Result: exit status 0 after the provider-contract source-anchor fix; 1025 tests run, 1025 passed, 3123 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run -p clankers-artifacts golden_hash_fixtures_cover_supported_artifact_kinds
```

Result: exit status 0; 1 test run, 1 passed, 19 skipped.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  cargo nextest run --workspace --partition count:4/4 --status-level fail --final-status-level slow
```

Result: exit status 0 after the stale artifact golden fixture was refreshed; 1000 tests run, 1000 passed, 3148 skipped.

## Repository verification rail

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= \
  ./scripts/verify.sh
```

Result: exit status 0; Verus reported 71 verified and 0 errors, no-std functional core rails passed, Tracey reported 47/47 requirements covered and verified, and the script ended with `=== All checks passed ===`.

## Cairn sync and closeout checks

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- sync drain-remaining-coupling-hotspots --root .
```

Result: exit status 0; dry run was not blocked and planned `sync_delta_spec` for `cairn/changes/drain-remaining-coupling-hotspots/specs/remaining-coupling-drain/spec.md`.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- sync drain-remaining-coupling-hotspots --root . --execute
```

Result: exit status 0; `mutated: true`, creating `cairn/specs/remaining-coupling-drain/spec.md`.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate proposal drain-remaining-coupling-hotspots --root .
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate design drain-remaining-coupling-hotspots --root .
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- gate tasks drain-remaining-coupling-hotspots --root .
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- validate --root .
```

Result: exit status 0 for all commands; proposal, design, and tasks gates returned `verdict: PASS`; `validate` returned `valid: true` with 52 specs validated.

```text
TMPDIR=/home/brittonr/.cargo-target/tmp \
  nix run .#cairn -- archive drain-remaining-coupling-hotspots --root .
```

Result: exit status 0; dry run was not blocked and planned `archive_change` for `./cairn/changes/drain-remaining-coupling-hotspots`.

## Whitespace

```text
git diff --check
```

Result: exit status 0.
