# Design: Observable Soak Rails

## Summary

The implementation keeps each focused rail as an executable Rust script and adds a harness-level `soak` mode that repeats rail selectors. The soak mode is intentionally a coordinator: it does not reinterpret rail-specific pass criteria, and it lets each dogfood rail emit its own receipt under `target/dogfood/*/receipt.json` while the harness emits the aggregate run receipt under `target/test-harness/`.

## Surfaces

- `dogfood streaming-tokens`: standalone real TUI token/thinking streaming, delayed local SSE provider, mid-stream follow-up proof.
- `dogfood daemon-attach-streaming-abort`: isolated daemon plus real attached TUI, long synthetic stream, follow-up submission while streaming, no daemon busy rejection, provider timing proof.
- `dogfood daemon-attach-reconnect`: isolated daemon attach, detach, reattach, session not forked, history replay visible.
- `dogfood bg-process-tui`: real TUI background process panel visibility and cleanup.

## Soak selector policy

`./scripts/test-harness.sh soak <selector> <iterations>` expands selectors into repeated dogfood steps:

- `all`: bg-process TUI, streaming tokens, daemon attach streaming abort, daemon attach reconnect.
- `tui`: bg-process TUI and streaming tokens.
- `daemon-attach`: daemon attach streaming abort and daemon attach reconnect.
- `streaming`: streaming tokens and daemon attach streaming abort.

Iterations must be a positive integer bounded to 1..50. If the argument is omitted, the harness uses `CLANKERS_SOAK_ITERATIONS` or 3.

## Receipt and observability policy

The aggregate harness receipt remains the source for soak step pass/fail. Each repeated dogfood rail remains responsible for its own proof fields and screen/log artifacts. A soak pass means every repeated step exited successfully; it does not replace inspecting the per-rail receipt fields when making a specific streaming, attach, reconnect, or process-panel claim.

## Verification plan

- Harness contract dry-run proves `soak streaming 2` expands to two iterations of the standalone and daemon/attach streaming rails.
- Docs/readiness tests prove the new commands and pass criteria are discoverable.
- Runtime dogfood evidence proves `daemon-attach-streaming-abort` and `streaming-tokens` pass against current code.
- Focused daemon actor and bounded-drain unit tests preserve the implementation seams behind the dogfood receipts.
