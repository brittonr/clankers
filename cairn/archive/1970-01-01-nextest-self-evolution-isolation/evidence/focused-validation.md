Evidence-ID: focused-validation
Artifact-Type: test-report
Task-ID: V1
Covers: r[nextest-self-evolution-isolation.verification.focused-rails]
Created: 2026-05-30
Status: complete

# Focused Validation

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run --test self_evolution_cli --test readiness_e2e
git diff --check
rustfmt --check tests/readiness_e2e.rs tests/self_evolution_cli.rs
```

## Results

```text
Nextest run ID 1f30e198-fa13-4827-bb69-73768994ee51
Starting 5 tests across 2 binaries
PASS clankers::readiness_e2e readiness_e2e_fake_provider_print_bash_read_find_and_json
PASS clankers::readiness_e2e readiness_e2e_fake_provider_write_edit_read_round_trip
PASS clankers::readiness_e2e readiness_e2e_version_help_config_and_auth_are_credential_free
PASS clankers::self_evolution_cli self_evolution_cli_rejects_stale_target_live_apply_before_mutation
PASS clankers::self_evolution_cli self_evolution_cli_runs_approve_preflight_and_live_apply_with_temp_files
Summary [1.955s] 5 tests run: 5 passed, 0 skipped
STATUS 0

git diff --check
STATUS 0

rustfmt --check tests/readiness_e2e.rs tests/self_evolution_cli.rs
STATUS 0
```
