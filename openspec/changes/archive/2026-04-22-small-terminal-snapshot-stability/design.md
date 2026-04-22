## Context

`tests/tui/visual.rs::snapshot_small_terminal` captures the startup TUI at 12x50 and asserts a structure snapshot stored in `tests/tui/snapshots/tui_tests__tui__visual__small_12x50_structure.snap`. A recent full-suite run failed on that snapshot with a small but user-visible structure diff, which means one of three things is true: the TUI layout regressed, the harness captured unstable startup state, or the checked-in snapshot no longer matches intentional behavior.

This change is intentionally separate from the active `support-openai-subscription-plans` work. The goal is to restore trust in the visual baseline without mixing TUI cleanup into the auth/Codex stream.

## Goals / Non-Goals

**Goals:**
- Determine whether the 12x50 snapshot failure is caused by real rendering/layout drift, unstable test capture, or stale snapshot expectations.
- Restore a deterministic small-terminal startup structure snapshot that passes both in focused runs and when reached from the broader suite.
- Keep the fix scoped to the small-terminal startup visual path and leave a clear maintenance rule for future snapshot updates.

**Non-Goals:**
- Redesigning the general TUI layout for all terminal sizes.
- Reworking the entire visual snapshot framework.
- Folding broader TUI cleanup or auth/Codex behavior into this change.

## Decisions

### 1. Treat the failure as an investigation with one of two allowed outcomes
The implementation first classifies the drift:
- **Real layout/rendering bug** → fix TUI code so the startup 12x50 structure matches intended behavior.
- **Intentional/current behavior with stale expectation** → update the checked-in snapshot after proving the rendered structure is deterministic and acceptable.

Rejected alternative: blindly update the snapshot on first diff. That would hide real regressions.

### 2. Keep the invariant at the startup-structure seam
The guarded contract is the startup structure captured by `snapshot_small_terminal`, not arbitrary screenshot pixels or unrelated interaction flows. The fix should keep validation centered on the existing structure snapshot seam, because it gives concise diffs and avoids brittle image-only review.

Rejected alternative: replacing the structure snapshot with only PNG review. That weakens regression detection.

### 3. Stabilize capture before broadening assertions
If the investigation shows startup state is unstable, the first fix is to stabilize the harness or assertion boundary for this test: wait for the intended startup state, exclude volatile text, or otherwise capture a stable post-settle frame. Only after that should the snapshot be refreshed.

Rejected alternative: adding more broad snapshot assertions before the seam is stable. That would multiply flaky failures.

### 4. Keep evidence local and explicit
Verification for this change should include both the focused small-terminal test and a broader visual/slash or full-suite run that proves the repaired snapshot no longer fails only when reached late in the test run. This distinguishes local determinism from suite-order sensitivity.

Rejected alternative: relying only on one isolated rerun. That can miss contamination or timing issues visible only in longer runs.

## Risks / Trade-offs

- **Overfitting to incidental startup text** → Prefer stable structural assertions and explicit settling over snapshotting transient content.
- **Refreshing the snapshot without proving determinism** → Require both focused and broader validation before accepting a new baseline.
- **Fixing the symptom in the harness when layout is actually broken** → Compare the rendered output with intended small-terminal layout before choosing harness-only changes.
- **Scope creep into general TUI cleanup** → Limit tasks to the 12x50 startup snapshot path and directly related seams.
