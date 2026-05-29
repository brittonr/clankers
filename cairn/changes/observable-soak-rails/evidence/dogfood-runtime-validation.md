Evidence-ID: observable-soak-rails-dogfood-runtime-validation
Task-ID: V2
Artifact-Type: validation-log
Covers: r[clankers-observable-soak-rails.daemon-attach-abort.followup-before-completion], r[clankers-observable-soak-rails.pi-observable-surface.named-receipts], r[clankers-observable-soak-rails.pi-observable-surface.local-stubs]
Status: pass

## Commands

```text
./scripts/test-harness.sh dogfood daemon-attach-streaming-abort
./scripts/test-harness.sh dogfood streaming-tokens
./scripts/test-harness.sh soak streaming 1
./scripts/test-harness.sh soak all 1
```

## Result summary

- Latest daemon/attach streaming-abort receipt from real all-surface soak: `target/dogfood/daemon-attach-streaming-abort-1780028707/receipt.json`.
  - `result: pass`
  - `provider_requests: 2`
  - `mid_stream_abort_processed_before_provider_returned: true`
  - `followup_request_started_before_stream_completed: true`
  - `busy_rejection_visible: false`
  - `daemon_cleaned_up: true`
  - `timings_ms.stream_completed: null`
  - `timings_ms.followup_started: 1807`
- Latest standalone streaming-token receipt from real all-surface soak: `target/dogfood/streaming-tokens-1780028642/receipt.json`.
  - `result: pass`
  - `provider_requests: 3`
  - `observed_incremental_text: true`
  - `observed_incremental_thinking: true`
  - `mid_stream_input_sent_before_response_returned: true`
  - `input_interrupt_timings_ms.interrupt_stream_completed: null`
  - `input_interrupt_timings_ms.followup_request_started: 7177`
- Background process TUI receipt from real all-surface soak: `target/dogfood/bg-process-tui-1780028568/receipt.json`.
  - `result: pass`
  - `active_processes_observed: 1`
  - `layout_toggle_bg_visible: true`
  - `command_visible: true`
  - `sentinel_processes_cleaned_up: true`
- Daemon attach reconnect receipt from real all-surface soak: `target/dogfood/daemon-attach-reconnect-1780028767/receipt.json`.
  - `result: pass`
  - `replayed_history_visible: true`
  - `session_not_forked: true`
  - `post_reattach_ack_visible: true`
  - `daemon_cleaned_up: true`

Both rails use deterministic local provider stubs and write screen artifacts under their receipt directories.
