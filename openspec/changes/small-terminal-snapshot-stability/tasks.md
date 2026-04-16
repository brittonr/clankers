## 1. Reproduce and classify the drift

- [ ] 1.1 Reproduce `tests/tui/visual.rs::snapshot_small_terminal` in both focused and broader test runs, and capture the observed structure diff
- [ ] 1.2 Determine whether the drift comes from TUI layout/rendering, harness timing/state, or a stale checked-in snapshot baseline
- [ ] 1.3 Record the accepted root cause and intended 12x50 startup layout contract in the change notes or implementation PR

## 2. Stabilize the 12x50 startup snapshot seam

- [ ] 2.1 Apply the minimal fix at the correct seam: TUI layout/rendering if behavior is wrong, or harness/assertion stabilization if capture is unstable
- [ ] 2.2 Update `tests/tui/visual.rs::snapshot_small_terminal` or nearby helpers so the asserted frame reflects stable startup layout only
- [ ] 2.3 Refresh `tests/tui/snapshots/tui_tests__tui__visual__small_12x50_structure.snap` only if the investigation proves the current deterministic layout is the correct baseline

## 3. Lock in regression coverage

- [ ] 3.1 Add or tighten focused regression checks so small-terminal startup layout drift fails with clear evidence
- [ ] 3.2 Run the focused small-terminal visual test and confirm it passes against the checked-in baseline without ad hoc snapshot regeneration
- [ ] 3.3 Run a broader automated test path that reaches `snapshot_small_terminal` and confirm the same baseline passes there too
