# Neutral service fixture evidence

Evidence-ID: neutral-fixtures
Artifact-Type: command-output-summary
Task-ID: V1
Covers: neutral-tool-service-context.verification.fixtures
Date: 2026-06-01
Status: PASS

## Commands

```text
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-agent
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo nextest run -p clankers-tool-host
```

## Relevant output

```text
clankers-agent: 193 tests run: 193 passed, 0 skipped
clankers-tool-host: 14 tests run: 14 passed, 0 skipped
```

## Notes

`turn::execution::tests::controller_tool_executor_runs_neutral_storage_search_progress_path` exercises a neutral-native controller tool path that requires storage and search services, emits neutral progress, and panics if the legacy runner is used. `clankers-tool-host` fixtures cover service success, missing service behavior, hook continue/modify/deny, capability denial, cancellation, progress emission, truncation, and explicit outcome variants.
