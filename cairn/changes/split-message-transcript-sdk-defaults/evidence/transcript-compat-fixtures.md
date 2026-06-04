# Transcript compatibility fixture evidence

Evidence-ID: split-message-transcript-sdk-defaults.transcript-compat-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: sdk-message-contract-boundary.transcript-compat-feature, sdk-message-contract-boundary.transcript-compat-feature.opt-in, sdk-message-contract-boundary.transcript-compat-feature.serialization
Date: 2026-06-04
Status: PASS

## Commands

```text
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clanker-message --features transcript-compat
env TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo check -p clanker-message --no-default-features
```

## Relevant output

```text
running 28 tests
...
test transcript::tests::agent_message_roundtrip ... ok
test transcript::tests::transcript_internal_serialization_fixture_survives ... ok
test transcript::tests::bash_execution_message_role ... ok
test transcript::tests::branch_summary_role ... ok
test transcript::tests::compaction_summary_role ... ok
test transcript::tests::custom_message_role ... ok
...
test result: ok. 28 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
exit=0

cargo check -p clanker-message --no-default-features
Finished `dev` profile
exit=0
```

## Compatibility coverage

The compatibility test run exercises persisted transcript records through the explicit `transcript-compat` feature, including user, assistant, tool-result, bash execution, branch summary, compaction summary, custom messages, IDs, timestamps, and usage helpers. The no-default-features check proves the stable SDK subset compiles without enabling those transcript records.
