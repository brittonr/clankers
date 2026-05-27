# Current-HEAD Release Evidence Index 2026-05-27

This page indexes the fresh current-HEAD full harness receipt for internal Clankers readiness review after native process restart support landed. It records local generated receipt paths for operator inspection; it does not publish raw logs, move readiness tags, or claim public unattended-production readiness.

## Payload binding

- Evaluated payload commit: `d9dff9a3d99a91382e18563b6db3dc67763b6ecd`
- Evaluated payload subject: `Implement native process restart`
- Evaluated payload branch/upstream: `main` / `origin/main`
- Evaluated payload describe: `internal-readiness-2026-05-26-dogfood-full-8-gd9dff9a3`
- Evaluated payload tracked dirty: `false`
- Evaluated payload ahead/behind at capture time: `1\t0`
- Evidence-recording commit: this checked-in page is committed after the harness run and is not itself the evaluated payload.

## Fresh full harness receipt

- Mode: `full`
- Run id: `20260527T044357Z-3274644`
- Started: `2026-05-27T04:43:57Z`
- Finished: `2026-05-27T04:58:16Z`
- Result: 8 passed / 0 failed / 0 skipped
- Results JSON: `target/test-harness/runs/20260527T044357Z-3274644/results.json`
- Summary: `target/test-harness/runs/20260527T044357Z-3274644/summary.md`
- Log directory: `target/test-harness/runs/20260527T044357Z-3274644/logs/`
- Latest aliases at capture time: `target/test-harness/results.json`, `target/test-harness/summary.md`, `target/test-harness/junit.xml`

Passed steps:

- `cargo fmt check` — `target/test-harness/runs/20260527T044357Z-3274644/logs/cargo_fmt_check.log`
- `cargo check tests` — `target/test-harness/runs/20260527T044357Z-3274644/logs/cargo_check_tests.log`
- `cargo nextest workspace` — `target/test-harness/runs/20260527T044357Z-3274644/logs/cargo_nextest_workspace.log`
- `cargo clippy` — `target/test-harness/runs/20260527T044357Z-3274644/logs/cargo_clippy.log`
- `repo verify` — `target/test-harness/runs/20260527T044357Z-3274644/logs/repo_verify.log`
- `tigerstyle` — `target/test-harness/runs/20260527T044357Z-3274644/logs/tigerstyle.log`
- `live readiness aspen2-qwen36` — `target/test-harness/runs/20260527T044357Z-3274644/logs/live_readiness_aspen2-qwen36.log`
- `dogfood bg-process-tui` — `target/test-harness/runs/20260527T044357Z-3274644/logs/dogfood_bg-process-tui.log`

## Generated local evidence index

The checked-in helper generated a local index under `target/release-evidence/current-head/`:

- Markdown index: `target/release-evidence/current-head/index.md`
- JSON index: `target/release-evidence/current-head/index.json`
- Full receipt selected there: `20260527T044357Z-3274644`
- Full receipt payload verification: `payload_commit_verified=true`
- Full results hash in generated index: `a7c55445558d15d28ac0a88cfc1076c1ec9d7205e0f560b3063993a5ae8ca6d4`
- Full summary hash in generated index: `016b59d31d6b8e017c34d20376a12b2f2ae75ca4a2c7aa52941ee00ca13a8c9e`

The helper run itself was recorded as mode `evidence-index` with run id `20260527T050202Z-3617999`; that run verifies the index-generation rail only and is not a replacement for the full harness receipt above.

## Readiness tag boundary

- Existing readiness tag: `internal-readiness-2026-05-26-dogfood-full`
- Tag object: `cc04004bfd7f6e6a45e3164c2f7890d71f6ee985`
- Peeled tagged payload: `ccec74b659dc588934378aed34638b333304695f`
- Tag movement: not moved by this evidence-index slice.
- Boundary note: the fresh evaluated payload `d9dff9a3d99a91382e18563b6db3dc67763b6ecd` is eight commits after the existing readiness tag; moving or creating another readiness tag is intentionally deferred to a separate operator decision.

## Non-claims

- This page indexes local generated evidence and receipt paths; it does not commit raw `target/test-harness/**` logs.
- It does not claim missing historical harness profiles as fresh passes; the generated index still reports stale/missing non-full modes separately.
- It does not claim public unattended-production readiness.
- It does not move or replace `internal-readiness-2026-05-26-dogfood-full`.
