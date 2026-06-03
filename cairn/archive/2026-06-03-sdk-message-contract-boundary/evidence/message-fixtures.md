# Message compatibility fixture evidence

Evidence-ID: sdk-message-contract-boundary-message-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: sdk-message-contract-boundary.verification.compat-fixtures
Date: 2026-06-03
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clanker-message
```

## Relevant output

```text
PASS clanker-message transcript::tests::transcript_internal_serialization_fixture_survives
Summary: 28 tests run: 28 passed, 0 skipped
```

## Coverage notes

`transcript_internal_serialization_fixture_survives` pins the serialized shape for a transcript-internal `AgentMessage::CompactionSummary` with `MessageId` values, compacted IDs, token count, and fixed RFC3339 timestamp. The full `clanker-message` nextest run also preserves the existing content, semantic event, result-streaming, and transcript role/ID/timestamp round-trip tests after the split into `content` and `transcript` modules.
