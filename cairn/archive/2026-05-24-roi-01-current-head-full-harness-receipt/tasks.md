# Tasks

- [x] [serial] T1. Run clean-state/current-HEAD preflight and record the intended payload commit. [covers=r[current-head-full-harness-receipt.payload-binding]]
- [x] [parallel] T2. Run `./scripts/test-harness.sh full` and preserve the generated result paths. [covers=r[current-head-full-harness-receipt.full-pass]]
- [x] [parallel] T3. Verify `results.json` and `summary.md` report full mode, zero failures, clean payload state, and the preflight payload commit. [covers=r[current-head-full-harness-receipt.payload-binding],r[current-head-full-harness-receipt.full-pass]]
- [x] [serial] T4. Leave generated `target/` artifacts untracked and report paths rather than committing logs. [covers=r[current-head-full-harness-receipt.non-publication]]
