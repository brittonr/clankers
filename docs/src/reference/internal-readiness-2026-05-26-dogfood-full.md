# Internal Readiness Checkpoint 2026-05-26 Dogfood Full

`internal-readiness-2026-05-26-dogfood-full` is an internal/trusted dogfood checkpoint for Clankers after the maintained background-process TUI dogfood rail was promoted into the normal full readiness harness. It is not a public unattended-production readiness claim.

## Identity

- Tag: `internal-readiness-2026-05-26-dogfood-full`
- Tag object: `cc04004bfd7f6e6a45e3164c2f7890d71f6ee985`
- Tagged payload commit: `ccec74b659dc588934378aed34638b333304695f`
- Tagged payload subject: `Promote BG process TUI dogfood to readiness`
- Scope: internal/trusted dogfood readiness for the full local harness, primary aspen2 Qwen live gate, and maintained real-TUI background-process dogfood rail.

## Full harness evidence

The full Clankers readiness harness passed against the tagged payload commit:

- Command: `./scripts/test-harness.sh full`
- Run id: `20260526T021502Z-3107712`
- Mode: `full`
- Started at: `2026-05-26T02:15:02Z`
- Finished at: `2026-05-26T02:26:23Z`
- Payload commit: `ccec74b659dc588934378aed34638b333304695f`
- Payload branch: `main`
- Payload describe: `internal-readiness-2026-05-26-1-gccec74b6`
- Payload tracked dirty: `false`
- Payload upstream: `origin/main`
- Payload ahead/behind: `0\t0`
- Steps passed: `8`
- Steps failed: `0`
- Steps skipped: `0`

Indexed local evidence paths:

- Full harness results: `target/test-harness/runs/20260526T021502Z-3107712/results.json`
- Full harness logs directory: `target/test-harness/runs/20260526T021502Z-3107712/logs/`
- Dogfood step log: `target/test-harness/runs/20260526T021502Z-3107712/logs/dogfood_bg-process-tui.log`
- BG-process TUI dogfood receipt: `target/dogfood/bg-process-tui-1779762368/receipt.json`

The generated `target/` receipts remain local operator evidence and are not checked into the repository.

## Harness steps included

The full harness run recorded these passing steps:

1. `cargo fmt --check`
2. `cargo check --tests`
3. `cargo nextest run --workspace --no-fail-fast`
4. `cargo clippy --workspace --all-targets -- -D warnings`
5. `./scripts/verify.sh`
6. `./xtask/tigerstyle.sh`
7. live `aspen2-qwen36` readiness
8. `dogfood bg-process-tui`

## BG-process TUI dogfood facts

The dogfood receipt for the full harness run recorded:

- Receipt schema: `clankers.bg_process_tui_dogfood.receipt.v1`
- Result: `pass`
- Active process title: `Spawned/BG (1 active)`
- Active processes observed: `1`
- Bounded command seconds: `12`
- Provider requests: `2`
- `/layout toggle bg` visibility: `true`
- Bounded command visibility: `true`
- Sentinel process cleanup: `true`
- Screen artifact before process: `target/dogfood/bg-process-tui-1779762368/screen-before-process.txt`
- Screen artifact with active process: `target/dogfood/bg-process-tui-1779762368/screen-active-process.txt`

These facts prove that the promoted full harness exercised the real Clankers TUI in tmux, used a deterministic local provider stub, started a bounded background process through the `process` tool, rendered the `Spawned/BG` panel with an active process, and cleaned up the sentinel process.

## What this checkpoint proves

- The readiness tag points at a clean, synced payload commit that passed the normal full harness.
- The normal full harness included the maintained BG-process TUI dogfood rail.
- The primary live aspen2 Qwen readiness gate passed in the same full harness run.
- The operator-visible background-process TUI path was exercised with a receipt that records panel visibility, command visibility, and cleanup.

## What this checkpoint does not prove

- It does not claim unattended public production readiness.
- It does not prove every provider/backend combination, terminal size, remote attach path, or long-running process shape.
- It does not prove readiness for commits after `ccec74b659dc588934378aed34638b333304695f`; newer commits require a fresh full harness receipt before a new readiness claim.
- It does not make generated `target/` receipts durable repository artifacts; this page indexes stable local paths and facts only.

## Evidence interpretation

This checkpoint is suitable for internal/trusted dogfooding because the annotated tag, payload metadata, full harness receipt, aspen2 Qwen live gate, and BG-process TUI dogfood receipt all agree on the same clean payload commit and successful full-readiness run.
