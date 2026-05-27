# Steel repo evolution pack verification

Artifact-Type: command-output-summary
Task-ID: V1, V2
Covers: steel-repo-evolution-packs.verification.fixtures, steel-repo-evolution-packs.verification.docs
Date: 2026-05-27
Status: PASS

## Focused commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime steel_repo_evolution --lib
./scripts/check-steel-repo-evolution-packs.rs
```

Relevant output:

```text
running 6 tests
steel_repo_evolution::tests::absent_pack_is_inactive ... ok
steel_repo_evolution::tests::valid_pack_activates_with_hashes_and_host_abi ... ok
steel_repo_evolution::tests::invalid_packs_fail_before_script_execution ... ok
steel_repo_evolution::tests::missing_and_hash_mismatched_scripts_fail_closed ... ok
steel_repo_evolution::tests::plan_accepts_known_host_calls_and_gates ... ok
steel_repo_evolution::tests::plan_rejects_malformed_unknown_host_and_unknown_gate ... ok
steel repo evolution pack receipt written to target/steel-repo-evolution-packs/receipt.json
```

The checker covers absent, valid, malformed, hash-mismatched, path-escaped, unknown-host-call, over-budget, valid-plan, and malformed-plan fixtures.

## Final validation commands

```text
mdbook build docs
nix run .#cairn -- gate proposal add-steel-repo-evolution-packs --root .
nix run .#cairn -- gate design add-steel-repo-evolution-packs --root .
nix run .#cairn -- gate tasks add-steel-repo-evolution-packs --root .
nix run .#cairn -- validate --root .
git diff --check
```

These commands passed after docs, runtime core, checker script, and Cairn task updates were present.
