Evidence-ID: basalt-fixtures
Artifact-Type: test-report
Task-ID: V2
Covers: r[ucan-basalt-daemon-auth.basalt-policy.mandatory], r[ucan-basalt-daemon-auth.receipts.redacted], r[ucan-basalt-daemon-auth.verification.basalt-fixtures]
Created: 2026-05-29
Status: complete

# Basalt Policy Fixture Verification

## Scope

Deterministic tests in `crates/clankers-ucan/src/basalt_authority.rs` exercise the shared `BasaltUcanAuthority` with local policy fixtures. The tests prove that public UCAN verification and Basalt policy must both allow the same normalized request, and that receipts remain redacted.

Covered cases:

- recognized `session-attach` contract/resource/ability allows when UCAN grant also allows
- resource outside policy denies despite a valid UCAN grant
- UCAN grant missing the requested ability denies before Basalt allow can matter
- unknown contract denies fail-closed
- unknown ability denies fail-closed
- receipts include redacted metadata (`token_reference`, `policy_hash`) and omit raw compact tokens/proofs

## Machine Evidence

Command run:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-ucan --lib
```

Result excerpt:

```text
test basalt_authority::tests::allows_when_public_ucan_and_basalt_policy_both_allow ... ok
test basalt_authority::tests::denies_when_policy_rejects_resource_even_with_valid_ucan ... ok
test basalt_authority::tests::denies_when_ucan_does_not_cover_requested_ability ... ok
test basalt_authority::tests::denies_unknown_contract_even_with_matching_public_ucan_grant ... ok
test basalt_authority::tests::denies_unknown_ability_even_with_matching_public_ucan_grant ... ok
test basalt_authority::tests::receipt_is_redacted ... ok

test result: ok. 86 passed; 0 failed
```
