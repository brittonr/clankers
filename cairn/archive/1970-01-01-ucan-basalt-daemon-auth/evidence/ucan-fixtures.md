Evidence-ID: ucan-fixtures
Artifact-Type: test-report
Task-ID: V1
Covers: r[ucan-basalt-daemon-auth.public-ucan.delegation-chain], r[ucan-basalt-daemon-auth.storage.replay-revocation], r[ucan-basalt-daemon-auth.verification.ucan-fixtures]
Created: 2026-05-29
Status: complete

# Public UCAN Fixture Verification

## Scope

Deterministic tests in `crates/clankers-ucan/src/public_credential.rs`, `public_store.rs`, and `basalt_authority.rs` cover public UCAN envelope decoding and verification edges:

- valid root, child, and grandchild delegation
- malformed base64/JSON and legacy `clanker-auth` base64 rejection
- unsupported schema rejection
- expired and not-before tokens
- wrong audience metadata
- missing proof chains
- untrusted roots and unreferenced trusted proofs
- widened child delegation fail-closed behavior
- replay-id duplicate denial and revoked proof-reference denial through the redb-backed public store

## Machine Evidence

Command run:

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers-ucan --lib
```

Result:

```text
running 86 tests
...
test public_credential::tests::envelope_supplies_proofs_for_child_delegation ... ok
test public_credential::tests::envelope_supplies_proofs_for_grandchild_delegation ... ok
test public_credential::tests::malformed_base64_and_json_are_rejected ... ok
test public_credential::tests::expired_and_not_before_tokens_are_rejected ... ok
test public_credential::tests::missing_proof_in_child_delegation_is_rejected ... ok
test public_credential::tests::wrong_audience_metadata_is_rejected_after_verification ... ok
test public_credential::tests::widened_child_delegation_fails_closed ... ok
test public_store::tests::credential_replay_is_admitted_once ... ok
test basalt_authority::tests::store_authorization_denies_duplicate_replay_id ... ok
test basalt_authority::tests::store_authorization_denies_revoked_reference ... ok

test result: ok. 86 passed; 0 failed
```
