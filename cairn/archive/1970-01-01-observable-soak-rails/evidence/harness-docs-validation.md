Evidence-ID: observable-soak-rails-harness-docs-validation
Task-ID: V1
Artifact-Type: validation-log
Covers: r[clankers-observable-soak-rails.soak-harness.streaming-expansion], r[clankers-observable-soak-rails.release-docs.no-overclaim]
Status: pass

## Commands

```text
CLANKERS_TEST_DRY_RUN=1 CLANKERS_TEST_RUN_ID=soak-final-dry-run-4 ./scripts/test-harness.sh soak streaming 02
./scripts/test-harness.sh soak streaming 1
./scripts/test-harness.sh soak all 1
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --test daemon_attach_streaming_abort_dogfood_docs --test streaming_tokens_recording_docs --test test_harness_contract --test release_readiness_docs
TMPDIR=/home/brittonr/.cargo-target/tmp RUSTC_WRAPPER= cargo test -p clankers --test test_harness_contract
```

## Result summary

- `soak streaming 2` dry-run expanded to four steps:
  - `soak streaming 1/2 streaming-tokens`
  - `soak streaming 1/2 daemon-attach-streaming-abort`
  - `soak streaming 2/2 streaming-tokens`
  - `soak streaming 2/2 daemon-attach-streaming-abort`
- Docs/contract tests passed, including the invalid-iteration and unknown-selector regression `test_harness_soak_rejects_invalid_inputs_before_running_steps`.
- Harness receipt from the final dry-run: `target/test-harness/runs/soak-final-dry-run-4/results.json`.
- The final dry-run used `02` and normalized labels to `1/2` and `2/2`, proving decimal-safe iteration handling.
- Real streaming soak receipt `target/test-harness/runs/20260529T041115Z-1716403/results.json` passed both `soak streaming 1/1 streaming-tokens` and `soak streaming 1/1 daemon-attach-streaming-abort`.
- Real all-surface soak receipt `target/test-harness/runs/20260529T042150Z-1752761/results.json` passed bg-process TUI, streaming tokens, daemon attach streaming abort, and daemon attach reconnect once each.
