# Steel orchestration-pack mutation verification

Artifact-Type: command-output-summary
Task-ID: V1, V2
Covers: steel-self-mutation-policy.verification-fixtures.orchestration-pack, steel-self-mutation-policy.host-functions.authority-kernel-checkpoint
Date: 2026-05-27
Status: PASS

## Focused commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime steel_orchestration_mutation --lib
./scripts/check-steel-orchestration-pack-mutation.rs
./scripts/check-steel-self-mutation-policy.rs
```

Relevant output:

```text
running 4 tests
steel_orchestration_mutation::tests::valid_orchestration_patch_stages_and_promotes_after_gates ... ok
steel_orchestration_mutation::tests::invalid_orchestration_patches_fail_before_writes ... ok
steel_orchestration_mutation::tests::failed_gate_blocks_activation_after_isolated_stage ... ok
steel_orchestration_mutation::tests::rollback_requires_current_and_backup_hash_match ... ok
steel orchestration-pack mutation receipt written to target/steel-orchestration-pack-mutation/receipt.json
steel self-mutation policy receipt written to target/steel-self-mutation/policy-receipt.json
```

The checker covers valid update, path escape, stale before hash, authority widening, required gate removal, failed validation, malformed schema, malformed patch hash, stale rollback, and guarded rollback fixtures.

## Final validation commands

```text
mdbook build docs
nix run .#cairn -- gate proposal allow-steel-orchestration-pack-mutation --root .
nix run .#cairn -- gate design allow-steel-orchestration-pack-mutation --root .
nix run .#cairn -- gate tasks allow-steel-orchestration-pack-mutation --root .
nix run .#cairn -- validate --root .
git diff --check
```

These commands passed after docs, runtime core, checker script, policy updates, and Cairn task updates were present.
