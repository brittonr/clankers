# Verification Evidence

Artifact-Type: verification-summary
Task-ID: V1,V2,V3
Covers: steel-repo-evolution-packs.runtime-turn-load.turn-path, steel-repo-evolution-packs.runtime-turn-load.absent, steel-repo-evolution-packs.higher-order-contracts.allowed-covered, steel-repo-evolution-packs.higher-order-contracts.plan-denied, steel-repo-evolution-packs.higher-order-contracts.nickel-source, steel-self-mutation-policy.host-functions.apply-through-rust, steel-self-mutation-policy.receipts-and-preflight.preflight, steel-self-mutation-policy.receipts-and-preflight.safe-receipt, steel-self-mutation-policy.verification-and-rollback.failed-verification, steel-self-mutation-policy.verification-and-rollback.guarded-rollback, steel-self-mutation-policy.verification-fixtures.positive, steel-self-mutation-policy.verification-fixtures.negative

## Commands and Results

### Repo evolution runtime/load rails

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime steel_repo_evolution --lib
```

Result excerpt:

```text
running 7 tests
test steel_repo_evolution::tests::invalid_nickel_and_missing_contracts_fail_closed ... ok
test steel_repo_evolution::tests::valid_pack_activates_with_hashes_and_host_abi ... ok
test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 150 filtered out; finished in 0.00s
```

Command:

```text
./scripts/check-steel-repo-evolution-packs.rs
```

Result:

```text
steel repo evolution pack receipt written to target/steel-repo-evolution-packs/receipt.json
```

The checker receipt includes fixtures for `repo-local-runtime-load`, `invalid-nickel-contract`, and higher-order host contracts. It hashes `.clankers/steel/evolution-profile.ncl`, `.clankers/steel/evolution-profile.json`, and `.clankers/steel/scripts/plan-evolution.scm`.

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clankers-agent --lib
```

Result excerpt:

```text
Checking clankers-agent v0.1.0 (/home/brittonr/git/clankers/crates/clankers-agent)
Finished `dev` profile [optimized + debuginfo] target(s) in 1.74s
```

This compiles the turn-path integration that calls `load_repo_evolution_pack(...)` from normal and orchestrated turn planning paths.

### Orchestration mutation isolated staging rails

Command:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-runtime steel_orchestration_mutation --lib
```

Result excerpt:

```text
running 5 tests
test steel_orchestration_mutation::tests::denied_receipts_redact_unsafe_content ... ok
test steel_orchestration_mutation::tests::valid_orchestration_patch_stages_and_promotes_after_gates ... ok
test steel_orchestration_mutation::tests::failed_gate_blocks_activation_after_isolated_stage ... ok
test steel_orchestration_mutation::tests::rollback_requires_current_and_backup_hash_match ... ok
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 153 filtered out; finished in 0.13s
```

Command:

```text
./scripts/check-steel-orchestration-pack-mutation.rs
```

Result:

```text
steel orchestration-pack mutation receipt written to target/steel-orchestration-pack-mutation/receipt.json
```

The checker uses `stage_orchestration_patch_to_directory(...)`, `promote_staged_orchestration_pack_to_directory(...)`, and `rollback_orchestration_pack_to_directory(...)`. It proves typed payloads write only below an isolated staging root after preflight validation, live promotion copies staged files only after hash guards, rollback restores backup files only after current/backup hash guards, raw write attempts fail before side effects, and unsafe receipt content is redacted, including malformed `b3:` patch-hash payloads and unsafe selected-gate payloads.

### Docs, Cairn, diff rails

Command:

```text
mdbook build docs && nix run .#cairn -- validate --root . && git diff --check
```

Result excerpt:

```text
INFO HTML book written to `/home/brittonr/git/clankers/docs/book`
{
  "change_issues": [],
  "changes": 0,
  "issues": [],
  "layout": "cairn",
  "policy": "cairn-default",
  "spec_issues": [],
  "specs_validated": 106,
  "valid": true
}
```

`git diff --check` produced no output and exited successfully.
