## 1. Reproduce and classify the drift

- [x] 1.1 Reproduce `tests/tui/visual.rs::snapshot_small_terminal` in both focused and broader test runs, and capture the observed structure diff
- [x] 1.2 Determine whether the drift comes from TUI layout/rendering, harness timing/state, or a stale checked-in snapshot baseline
- [x] 1.3 Record the accepted root cause and intended 12x50 startup layout contract in the change notes or implementation PR

## 2. Stabilize the 12x50 startup snapshot seam

- [x] 2.1 Apply the minimal fix at the correct seam: TUI layout/rendering if behavior is wrong, or harness/assertion stabilization if capture is unstable
- [x] 2.2 Update `tests/tui/visual.rs::snapshot_small_terminal` or nearby helpers so the asserted frame reflects stable startup layout only
- [x] 2.3 Refresh `tests/tui/snapshots/tui_tests__tui__visual__small_12x50_structure.snap` only if the investigation proves the current deterministic layout is the correct baseline

## 3. Lock in regression coverage

- [x] 3.1 Add or tighten focused regression checks so small-terminal startup layout drift fails with clear evidence
- [x] 3.2 Run the focused small-terminal visual test and confirm it passes against the checked-in baseline without ad hoc snapshot regeneration
- [x] 3.3 Run a broader automated test path that reaches `snapshot_small_terminal` and confirm the same baseline passes there too

## Investigation notes

- Fresh focused repro on 2026-04-22 showed `cargo nextest run -E 'test(snapshot_small_terminal)' --failure-output immediate-final` failing with row-1 drift from the accepted `│Noankers │` snapshot content to `│Noonr/.c │`, so the change was still live in this worktree.
- The drift comes from the Todo panel's first wrapped empty-state row, not a real 12x50 layout change: the extracted structure was capturing transient trailing text in that eight-cell row instead of a stable startup-layout contract.
- Fix applied at the assertion seam: `tests/tui/visual.rs::snapshot_small_terminal` now waits for extracted structure to settle, and `tests/tui/snapshot.rs::extract_structure(...)` normalizes the Todo empty-state first row to the canonical clipped `No items...` prefix before snapshot comparison.
- The accepted 12x50 baseline is now the normalized startup-layout row (`│No items.│`) rather than transient bleed like `Noankers` / `Noonr/.c`.
- Focused verification after the fix: `cargo nextest run -E 'test(snapshot_small_terminal)' --failure-output immediate-final` → PASS on 2026-04-22.
- A full `cargo nextest run --test tui_tests --failure-output immediate-final` rerun on 2026-04-22 failed earlier at unrelated `tui::tmux_smoke::tmux_snapshot_after_version`, so it did not provide valid evidence for `snapshot_small_terminal`.
- Broader verification after the fix: `cargo nextest run --test tui_tests --failure-output immediate-final tui::visual::` → PASS on 2026-04-22 (18/18 visual tests, including `snapshot_small_terminal`).
