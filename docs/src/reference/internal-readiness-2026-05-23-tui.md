# Internal Readiness Checkpoint 2026-05-23 TUI

`internal-readiness-2026-05-23-tui` is an internal/trusted dogfood checkpoint for Clankers after the TUI spawned/background jobs panel was promoted into the default layout. It is not a public unattended-production readiness claim.

## Identity

- Tag: `internal-readiness-2026-05-23-tui`
- Tag object: `ad20bd256dc45694c19f8bc92635fc77c5d98eab`
- Tagged payload commit: `45815ba13df24b7c9b3c22f769fe52fc588f6350`
- Tagged payload subject: `Update tmux TUI snapshots for spawned jobs panel`
- Implementation commit: `f6dcb308 Show spawned background jobs in default TUI layout`
- Scope: internal/trusted dogfood readiness for the default TUI `Spawned/BG` process panel, its layout aliases, and current readiness harness coverage.

## What this checkpoint proves

- The default TUI layout includes the spawned/background jobs panel as `Spawned/BG`.
- `/layout toggle bg` hides the process panel.
- `/layout toggle spawned` shows the same process panel again.
- Leader-menu/help and visual snapshot surfaces were refreshed for the new default panel.
- The full Clankers readiness harness passed against the tagged payload commit:
  - mode: `full`;
  - run: `20260523T154823Z-1967230`;
  - payload commit: `45815ba13df24b7c9b3c22f769fe52fc588f6350`;
  - payload tracked dirty: `false`;
  - `7` steps passed, `0` failed, `0` skipped.
- The full harness run included:
  - `cargo fmt --check`;
  - `cargo check --tests`;
  - `cargo nextest run --workspace --no-fail-fast`;
  - `cargo clippy --workspace --all-targets -- -D warnings`;
  - `./scripts/verify.sh`;
  - `./xtask/tigerstyle.sh`;
  - live `aspen2-qwen36` readiness.
- A live TUI dogfood run used the current built binary at `/home/brittonr/.cargo-target/debug/clankers` from a clean/synced checkout and a local OpenAI-compatible Qwen backend.
- The live dogfood run executed a real bash tool command: `sleep 60; echo BG_DOGFOOD_DONE`.
- During that run, the TUI rendered `Spawned/BG (1 active)` with the tracked command shown in the process table.
- The command completed and the assistant/tool transcript returned `BG_DOGFOOD_DONE`.
- The dogfood tmux session was cleaned up and the repository remained clean/synced afterward.

## What this checkpoint does not prove

- It does not claim unattended public production readiness.
- It does not prove every terminal size, theme, remote attach path, or provider/backend combination has been live-dogfooded.
- It does not make ignored `target/` captures durable release artifacts; they are local operator evidence indexed by this note.
- It does not claim GitHub Actions coverage; no recent Actions runs were visible during capture.

## Evidence locations

- Full harness summary: `target/test-harness/runs/20260523T154823Z-1967230/summary.md`
- Full harness latest summary copy: `target/test-harness/summary.md`
- Active dogfood capture: `target/bg-panel-active-count.txt`
- Final dogfood capture: `target/bg-panel-final.txt`
- Toggle-off capture: `target/bg-panel-toggle-bg-off.txt`
- Toggle-on capture: `target/bg-panel-toggle-spawned-on.txt`

## Evidence interpretation

This checkpoint is suitable for internal/trusted dogfooding of the default TUI spawned/background jobs panel because the code slice, snapshot maintenance, full readiness harness, pushed annotated tag, and live operator dogfood all agree on the same payload commit and behavior.

The remaining risk is breadth, not this slice's basic correctness: future work should expand real-operator coverage across remote attach, alternate terminal sizes, and longer-lived concurrent jobs if those paths become readiness-critical.
